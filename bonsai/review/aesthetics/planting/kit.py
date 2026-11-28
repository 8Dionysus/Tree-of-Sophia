#!/usr/bin/env python3
"""The planting kit of the Aesthetics bonsai: one command line for a packet's sources, its cross-check and its gate.

A packet is one work under packets/<work id>/ (AGENTS.md beside this file says what it holds). The kit fetches a
transcription and the page-indexed OCR of a scan, cuts a place out of the transcription by its opening and closing
words, compares every place word by word with the OCR, keeps the reader's judgment of every difference, and decides
whether the packet may go on (gate). Nothing here calls a model, and the gate recomputes what it can from the fetched
files, so a packet stands on what the files give and never on a reader's say-so.

  kit.py queue                         what is left to plant, written to queue.json
  kit.py open WORK                     make the packet and print its task
  kit.py task PACKET                   print the task again
  kit.py fetch PACKET ROLE URL         fetch a source into the cache (ROLE: transcription | other)
  kit.py ia PACKET ITEM [--long-s]     fetch the page-indexed OCR of an Internet Archive scan
  kit.py scan PACKET [ITEM]            the scans the packet reads (a work in several volumes has several); ITEM makes one the working scan
  kit.py find PACKET WORDS [--in FILE] where the words stand in the OCR of each scan, or in a fetched file
  kit.py leaf PACKET N                 the OCR of one leaf
  kit.py image PACKET N [--loc LOC]    fetch the image of page n<N> (the reader then looks at the file)
  kit.py offset PACKET K               image number minus OCR leaf number, read off one image
  kit.py person PACKET ROLE QID        a hand in the text (author | editor | translator) by its Wikidata item: name and year of death are read there
  kit.py rights PACKET                 write the witness's rights sentences from those dates
  kit.py why PACKET FILE REASON        why the words come from a transcription outside the proofreading projects
  kit.py printed PACKET YEAR           the year of the printing whose scan is read
  kit.py cut PACKET FILE OPEN CLOSE --loc LOC [--drop REGEX]   write a place: the run of the transcription from OPEN to CLOSE
  kit.py compare PACKET                compare every place with the OCR and list the differences
  kit.py judge PACKET LOC N|all|whole transcription|print [NOTE]
  kit.py correct PACKET LOC FROM TO EVIDENCE    the printed page reads otherwise: change the text and record it
  kit.py gate PACKET [--second] [--offline]
  kit.py settle PACKET                 move what the second reader does not confirm out of the reading
  kit.py accept PACKET --by ID [--basis TEXT]
  kit.py status
"""
import argparse,bisect,datetime,difflib,gzip,hashlib,html.parser,io,json,math,os,re,subprocess,sys,tempfile,time,unicodedata,urllib.parse,urllib.request
from pathlib import Path

HERE=os.path.dirname(os.path.abspath(__file__));REVIEW=os.path.dirname(HERE);ROOT=os.path.abspath(os.path.join(REVIEW,'..','..','..'))
PACKETS=os.path.join(HERE,'packets');HOME=os.path.join(ROOT,'bonsai','aesthetics')
CACHE=os.environ.get('TOS_PLANTING_CACHE') or os.path.join(os.path.expanduser('~'),'.cache','tos-planting')   # fetched files stay outside the checkout
# Queue classifications are leads, never a rights determination: edition and every hand
# must still be established at the gate. Unknown war participation uses the longer term.
def death_year(person):
    """The year a personal term runs from: the recorded death, or, for a hand of antiquity or the Middle Ages
    whose death is known only approximately, the documented upper bound of it (`died_before` with `death_bound`)."""
    if type(person.get('died')) is int:return person['died']
    bound=person.get('death_bound')
    if person.get('died') is None and type(person.get('died_before')) is int and person['died_before']<=BOUND_BEFORE and isinstance(bound,dict) and bound.get('from'):return person['died_before']
    return None

def public_domain_by_dates(persons,published,year=None):
    year = year if year is not None else datetime.date.today().year
    valid = bool(persons) and all(type(death_year(p)) is int for p in persons)
    def russian(person):
        died = death_year(person)
        rehabilitation = person.get('rehabilitated')
        if rehabilitation is not None:
            return type(rehabilitation) is int and rehabilitation <= year - (75 if person.get('wartime') is not False else 71)
        return died < 1943 or died <= year - (75 if person.get('wartime') is not False else 71)
    return {'russia':valid and all(russian(p) for p in persons),
            'usa':type(published) is int and published <= year - 96,
            'eu':valid and all(death_year(p) <= year - 71 for p in persons)}

BOUND_BEFORE=1800                                   # an upper bound of a death stands in for the year only for a hand this old
LONGEST_LIFE=122                                    # the longest attested human life: one alive in a year has died within this many years
BASES={'public-domain','public-domain+licence','licensed','short-quotation','locator-only'}
JURISDICTIONS=('russia','usa','eu')
LONGEST=2400                                         # a place is a formula, not a page
AGENT='ToS-planting/1 (+https://github.com/8Dionysus/Tree-of-Sophia)'

def load(path):
    with open(path,encoding='utf-8') as file:return json.load(file)
def save(path,value):
    with open(path,'w',encoding='utf-8') as file:file.write(json.dumps(value,ensure_ascii=False,indent=1)+'\n')
def layer(name):return load(os.path.join(REVIEW,'layer',f'{name}.json'))
def die(message):print(message,file=sys.stderr);sys.exit(1)
def today():return datetime.date.today().isoformat()
def slug(text):
    text=unicodedata.normalize('NFKD',text.lower());text=''.join(ch for ch in text if not unicodedata.combining(ch))
    return re.sub(r'^-|-$','',re.sub(r'[^a-z0-9]+','-',re.sub(r"[’']s\b",'s',text)))

# ---- the layer and what is left of it ----------------------------------------------------------------------------

def world(but=None):
    """The layer, the field as planted by every reader but one, and what every work still waits for."""
    field=load(os.path.join(HOME,'field.json'));fragments=load(os.path.join(HOME,'fragments.json'))
    concepts=layer('concepts');titles=load(os.path.join(REVIEW,'inputs','concepts.json'))['titles']
    canon={key:concept['key'] for concept in concepts for key in concept['keys']}
    field['conceptions']=[record for record in field['conceptions'] if record['reading']['reader']!=but];field['links']=[record for record in field['links'] if record['reading']['reader']!=but]
    grown={key for record in field['conceptions'] for key in record['grew_from']}|{key for record in field['links'] for key in record['grew_from']}
    return {'field':field,'fragments':fragments,'works':{work['id']:work for work in layer('works')},'agents':{agent['id']:agent for agent in layer('agents')},
            'lives':layer('lives'),'concepts':{concept['key']:concept for concept in concepts},'canon':canon,'titles':titles,'readings':layer('readings'),'links':layer('links'),
            'areas':{area['id'] for area in load(os.path.join(REVIEW,'inputs','areas.json'))['areas']},'grown':grown}

def task(w,work):
    """What a maker is asked: the work, the register readings and links of it that no conception has grown from."""
    known=w['works'].get(work) or die(f'no work {work} in the layer')
    concept=lambda key:{'key':w['canon'].get(key,key),'ru':w['concepts'][w['canon'].get(key,key)]['title']['ru'],'slug':slug(w['titles'][w['canon'].get(key,key)])}
    readings=[{'key':r['key'],'areas':r['areas'],'concepts':[concept(key) for key in r['concepts']],'locus':r.get('locus'),'thesis':r.get('thesis'),
               **({'distinction':r['distinction']} if r.get('distinction') else {}),'basis':r.get('basis'),**({'url':r['url']} if r.get('url') else {})}
              for r in w['readings'] if r['work']==work and r['key'] not in w['grown']]
    links=[{'key':l['key'],'from':concept(l['from']['concept']),'to':concept(l['to']['concept']),'verb':l.get('verb'),'grounds':l.get('grounds'),'mode':l.get('mode'),'basis':l.get('basis')}
           for l in w['links'] if work in (l.get('works') or []) and l['key'] not in w['grown'] and l['from'].get('concept') and l['to'].get('concept')]
    held=w['fragments']['works'].get(work)
    return {'work':work,'title':known['title'],'year':known.get('year'),'dates':known.get('dates'),'languages':known.get('languages'),'notes':known.get('notes'),
            'authors':[{'id':agent,'name':w['agents'][agent]['name'],'dates':w['agents'][agent].get('dates')} for agent in known['agents']],
            'readings':readings,'links':links,
            'already':{'original':held['original'],'witness':held['witnesses'][held['original']]['edition'],'places':list(w['fragments']['places'].get(work,{}))} if held else None,
            'conceptions_planted':[record['id'] for record in w['field']['conceptions'] if record['work']==work]}

def wave(w,work):
    """Prioritize leads by rights; the edition and all its hands are decided by the gate."""
    known=w['works'][work];packet_path=os.path.join(PACKETS,work)
    candidate=part(packet_path,'places.json') or {};entry=candidate.get('entry') or w['fragments']['works'].get(work) or {}
    witness=(entry.get('witnesses') or {}).get(entry.get('original')) or {};rights=witness.get('rights') or {}
    bases=[(rights.get(country) or {}).get('basis') if isinstance(rights.get(country),dict) else None for country in JURISDICTIONS]
    if all(basis in BASES for basis in bases):
        if all(basis=='locator-only' for basis in bases):return 'locator-only'
        free=sum(basis in ('public-domain','public-domain+licence','licensed') for basis in bases)
        return 'free-all' if free==3 else 'free-some' if free else 'quotation'
    people=[]
    for agent in known['agents']:
        life=(w.get('lives') or {}).get(agent) or {};dates=life.get('died') or []
        years=[]
        if life.get('item') and dates and all(type(item.get('precision')) is int and item['precision']>=9 and not item.get('approximate') for item in dates):
            for item in dates:
                match=re.match(r'([+-]\d+)-',str(item.get('time','')))
                if match:years.append(int(match.group(1)))
        if not years and not life.get('item'):
            notes=w['agents'][agent].get('dates') or []
            match=re.fullmatch(r'\s*\d{4}\s*[–-]\s*(\d{4})\s*',notes[0]) if notes else None
            if match:years.append(int(match.group(1)))
        people.append({'died':max(years) if years else None})
    rights=public_domain_by_dates(people,known.get('year'))
    return 'free-all' if all(rights.values()) else 'free-some' if any(rights.values()) else 'quotation'

def cmd_queue(args):
    w=world();rows=[];hold=load(os.path.join(HERE,'hold.json'))
    for work in w['works']:
        t=task(w,work)
        if not t['readings']:continue
        unread=all(r['basis']=='bibliography' for r in t['readings'])      # a catalogue stands behind every waiting reading: the work itself is still to be read
        corpus=any(record['work']==work and not record['formula']['cite'][0].get('loc') for record in w['field']['conceptions'])
        status='held' if work in hold else 'unread' if unread else 'planted' if work in w['fragments']['works'] else 'corpus' if corpus else 'waiting'
        rows.append({'work':work,'wave':wave(w,work),'status':status,'year':t['year'],'readings':len(t['readings']),'primary':sum(r['basis']=='primary' for r in t['readings']),
                     'links':len(t['links']),'areas':sorted({area for r in t['readings'] for area in r['areas']})})
    first=lambda row:sum(area in('AE01','AE02') for area in row['areas'])
    order={'free-all':0,'free-some':1,'quotation':2,'locator-only':3}
    rows.sort(key=lambda row:(order[row['wave']],-first(row),-row['readings'],row['work']))
    save(os.path.join(HERE,'queue.json'),{'made':'kit.py queue','rule':'Rights waves are leads from the register and sourced lives, not admission: the gate checks the actual edition and every hand. Status records held, unread, corpus and planted work separately.','works':rows})
    count={}
    for row in rows:count.setdefault(row['wave'],[0,0]);count[row['wave']][0]+=1;count[row['wave']][1]+=row['readings']
    print('queue.json: '+'; '.join(f'{name}: {works} works, {readings} readings' for name,(works,readings) in count.items()))

# ---- a packet ----------------------------------------------------------------------------------------------------

def packet(arg):
    path=arg if os.path.isdir(arg) else os.path.join(PACKETS,arg)
    if not os.path.isdir(path):die(f'no packet at {path}')
    return os.path.abspath(path)
def work_of(path):
    directory=Path(path)
    return directory.parent.parent.name if directory.parent.name=='extensions' else directory.name
def packet_selector(path):
    directory=Path(path)
    return f'{work_of(path)}/extensions/{directory.name}' if directory.parent.name=='extensions' else work_of(path)
def cache(path,*names):
    home=os.path.join(CACHE,packet_selector(path));os.makedirs(home,exist_ok=True);return os.path.join(home,*names)

def scoped_task(path,t):
    scope=part(path,'scope.json')
    if not scope:return t
    keys=set(scope.get('keys') or [])
    return {**t,'readings':[item for item in t['readings'] if item['key'] in keys],
            'links':[item for item in t['links'] if item['key'] in keys]}

def part(path,name,default=None):
    file=os.path.join(path,name);return load(file) if os.path.exists(file) else default
def evidence(path):return part(path,'evidence.json',{'sources':[],'scan':{},'persons':[],'published':None,'images':[],'places':{}})

def cmd_open(args):
    w=world();work=args.work;t=task(w,work)
    if not t['readings']:die(f'{work}: no register reading waits')
    path=os.path.join(PACKETS,work);os.makedirs(path,exist_ok=True);cache(path)
    known=w['works'][work];title={key:known['title'].get(key,'') for key in('ru','en')}
    if not os.path.exists(os.path.join(path,'places.json')):
        entry=None if t['already'] else {'author':{key:', '.join(a['name'].get(key,'') for a in t['authors']) for key in('ru','en')},'title':title,'original':'','witnesses':{},'absent':[]}
        save(os.path.join(path,'places.json'),{'work':work,**({'entry':entry} if entry else {}),'places':{},'corrections':[],'checked_on_the_page_image':[]})
    if not os.path.exists(os.path.join(path,'reading.json')):
        save(os.path.join(path,'reading.json'),{'reader':{'id':f'terra-{today()}-{slug(work.split(".")[-1])[:40]}','model':'gpt-6.1-terra','date':today(),'reads':''},'note':'',
            'library':{'agents':{a['id']:{'tree':f"tos.agent.{a['id']}"} for a in t['authors']},'works':{work:{'title':title,'original':t['already']['original'] if t['already'] else '','tree':f'tos.work.{work}','places':'fragments'}}},
            'conceptions':[],'links':[],'left':[]})
    if not os.path.exists(os.path.join(path,'evidence.json')):save(os.path.join(path,'evidence.json'),evidence(path))
    save(cache(path,'task.json'),t);print(path);show(t)

def show(t):
    print(f"\nWORK {t['work']}\n  {json.dumps(t['title'],ensure_ascii=False)}  year {t['year']}  dates {t['dates']}  languages {t['languages']}")
    for a in t['authors']:print(f"  author {a['id']}: {json.dumps(a['name'],ensure_ascii=False)} {a['dates']}")
    for note in t['notes'] or []:print(f'  note: {note}')
    if t['already']:print(f"  ALREADY IN THE BONSAI in {t['already']['original']}: {t['already']['witness']}\n  places held: {', '.join(t['already']['places'])}\n  conceptions planted: {', '.join(t['conceptions_planted'])}")
    print(f"\nREGISTER READINGS THAT WAIT ({len(t['readings'])})")
    for r in t['readings']:
        print(f"- {r['key']}  areas {' '.join(r['areas'])}  basis {r['basis']}  locus: {r['locus']}")
        print('    concepts: '+'; '.join(f"{c['key']} «{c['ru']}» ({c['slug']})" for c in r['concepts']))
        print(f"    thesis: {(r['thesis'] or {}).get('ru')}")
        if r.get('distinction'):print(f"    distinction: {r['distinction'].get('ru')}")
        if r.get('url'):print(f"    read at: {r['url']}")
    print(f"\nREGISTER LINKS THAT WAIT ({len(t['links'])})")
    for l in t['links']:print(f"- {l['key']}  {l['from']['key']} «{l['from']['ru']}» ({l['from']['slug']}) —[{l['verb']}]→ {l['to']['key']} «{l['to']['ru']}» ({l['to']['slug']})  grounds {l['grounds']}  {l['mode']}")

def cmd_task(args):
    path=packet(args.packet);t=scoped_task(path,task(world(),work_of(path)));save(cache(path,'task.json'),t);show(t)

# ---- sources -----------------------------------------------------------------------------------------------------

def get(url,tries=3):
    # urllib requires an ASCII URL; preserve existing escapes and URL delimiters.
    request_url=urllib.parse.quote(url,safe=":/?#[]@!$&'()*+,;=%")
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(request_url,headers={'User-Agent':AGENT}),timeout=90) as response:return response.read()
        except Exception as error:
            if attempt==tries-1:die(f'could not fetch {url}: {error}')
            time.sleep(3*(attempt+1))

class Plain(html.parser.HTMLParser):
    """The running text of an HTML transcription: page marks, footnote anchors and notes left out, blocks as paragraphs."""
    SKIP=re.compile(r'pagenum|pageno|fnanchor|footnote|linenum|noprint|reference|mw-editsection');BLOCK={'p','div','h1','h2','h3','h4','h5','h6','li','blockquote','tr','td','pre','table','section','hr','dd','dt'}
    def __init__(self):super().__init__();self.out=[];self.stack=[]
    def handle_starttag(self,tag,attrs):
        if tag in('br','img','hr','meta','link','input'):
            if tag=='br' and not any(self.stack):self.out.append('\n')
            if tag=='hr' and not any(self.stack):self.out.append('\n\n')
            return
        classes=dict(attrs).get('class') or ''
        skip=tag in('script','style','head','sup','title') or bool(self.SKIP.search(classes)) or 'toc' in classes.split()
        self.stack.append((tag,skip or bool(self.stack and self.stack[-1][1])))
        if tag in self.BLOCK and not self.stack[-1][1]:self.out.append('\n\n')
    def handle_endtag(self,tag):
        for index in range(len(self.stack)-1,-1,-1):
            if self.stack[index][0]==tag:
                hidden=self.stack[index][1];del self.stack[index:]
                if tag in self.BLOCK and not hidden:self.out.append('\n\n')
                return
    def handle_data(self,data):
        if not(self.stack and self.stack[-1][1]):self.out.append(data)

class MediaWikiPlain(Plain):
    """Limit a MediaWiki transcription to its actual rendered content."""
    def __init__(self):super().__init__();self.active=False;self.found=False
    def handle_starttag(self,tag,attrs):
        if not self.active and 'mw-parser-output' in (dict(attrs).get('class') or '').split():self.active=True;self.found=True
        if self.active:super().handle_starttag(tag,attrs)
    def handle_endtag(self,tag):
        if self.active:
            super().handle_endtag(tag)
            if not self.stack:self.active=False
    def handle_data(self,data):
        if self.active:super().handle_data(data)

def declared_charset(raw):
    """The character set an HTML page declares in its first bytes, if any."""
    head=raw[:4096].decode('ascii',errors='ignore')
    m=re.search(r'<meta[^>]+charset\s*=\s*["\']?([A-Za-z0-9_.:-]+)',head,re.I)
    return m.group(1).lower() if m else None

def plain(raw,name):
    charset=declared_charset(raw) if re.search(r'\.x?html?$',name) or re.match(rb'\s*<(!doctype|html)',raw[:300],re.I) else None
    try:text=raw.decode(charset,errors='strict') if charset and charset not in ('utf-8','utf8') else raw.decode('utf-8',errors='replace')
    except (LookupError,UnicodeDecodeError):text=raw.decode('utf-8',errors='replace')
    if re.search(r'\.x?html?$',name) or re.match(r'\s*<(!doctype|html)',text[:300],re.I):
        parser=MediaWikiPlain();parser.feed(text)
        if not parser.found:parser=Plain();parser.feed(text)
        text=''.join(parser.out)
    text=unicodedata.normalize('NFC',text.replace('\r\n','\n').replace('\r','\n').replace('­','').replace('﻿',''))
    return re.sub(r'\n[ \t]*\n\s*','\n\n',re.sub(r'[ \t   ]+',' ',text))

def record(path,role,url,name,raw,**more):
    with open(cache(path,name),'wb') as file:file.write(raw)
    ev=evidence(path);ev['sources']=[source for source in ev['sources'] if source['file']!=name]
    ev['sources'].append({'role':role,'url':url,'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'fetched':today(),**more})
    save(os.path.join(path,'evidence.json'),ev);return ev

SHADOW_HOSTS=('annas-archive.org','annas-archive.li','annas-archive.se','libgen.is','libgen.rs','libgen.li','libgen.st','z-lib.org','z-library.sk','zlibrary.to','dokumen.pub')
def lawful_url(url):
    parsed=urllib.parse.urlparse(str(url));host=(parsed.hostname or '').lower().rstrip('.')
    return parsed.scheme=='https' and bool(host) and not any(host==denied or host.endswith('.'+denied) for denied in SHADOW_HOSTS)

def cmd_fetch(args):
    if not lawful_url(args.url):die('source must be a lawful https URL, not a shadow library')
    path=packet(args.packet);name=args.name or re.sub(r'[^A-Za-z0-9._-]+','-',urllib.parse.urlparse(args.url).path.strip('/').split('/')[-1] or 'source')
    raw=open(args.file,'rb').read() if args.file else get(args.url)
    if name.endswith('.gz'):raw=gzip.decompress(raw);name=name[:-3]
    if args.role=='transcription' and raw.startswith(b'%PDF-'):die('fetch PDF bytes as other, then derive their text with kit.py pdf-text')
    more={}
    if args.role=='transcription':
        text=plain(raw,name);
        with open(cache(path,name+'.txt'),'w',encoding='utf-8') as file:file.write(text)
        more={'text':name+'.txt'}
    record(path,args.role,args.url,name,raw,**more)
    print(f"{name}: {len(raw)} bytes"+(f"; running text in {cache(path,name+'.txt')} ({len(text)} characters)" if more else ''))

def pdf_text(raw):
    if not raw.startswith(b'%PDF-'):die('the fetched source is not a PDF')
    try:result=subprocess.run(['pdftotext','-layout','-','-'],input=raw,capture_output=True,check=True)
    except (OSError,subprocess.CalledProcessError) as error:die(f'PDF text extraction failed: {error}')
    if not result.stdout.strip():die('the PDF has no extractable text; use a page-image reading')
    return result.stdout

def cmd_pdf_text(args):
    path=packet(args.packet);data=witness_data(path,args.language) if args.language else evidence(path)
    if args.language and os.path.isfile(witness_file(path,args.language)+'.accepted'):die('an accepted witness cannot be changed')
    if not args.language and part(path,'accepted.json') is not None:die('an accepted packet cannot be changed')
    sources=data.get('sources') or [];source=next((s for s in sources if s.get('file')==args.source and not s.get('derived_from')),None)
    if not source or not lawful_url(source.get('url')):die('fetch the PDF source first')
    local=(lambda name:witness_cache(path,args.language,name)) if args.language else (lambda name:cache(path,name))
    raw=Path(local(source['file'])).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=source.get('sha256'):die('the fetched PDF has changed')
    text=pdf_text(raw);digest=hashlib.sha256(text).hexdigest();name='pdf-text-'+hashlib.sha256((source['url']+'\0'+source['file']).encode()).hexdigest()[:16]+'-'+source['sha256']+'-'+digest+'.txt'
    version=subprocess.run(['pdftotext','-v'],capture_output=True,text=True).stderr.splitlines()[0]
    Path(local(name)).write_bytes(text)
    derived={'file':name,'url':source['url'],'sha256':digest,'fetched':today(),'derived_from':{'file':source['file'],'sha256':source['sha256'],'tool':'pdftotext','version':version,'options':['-layout']}}
    if not args.language:derived.update({'role':'transcription','text':name})
    data['sources']=[s for s in sources if s.get('file')!=name]+[derived]
    if args.language:data['review']={};save(witness_file(path,args.language),data)
    else:save(os.path.join(path,'evidence.json'),data)
    print(f'{name}: derived from {source["file"]}; compare with the independently inspected page')

def render_pdf_page(raw, page):
    if not raw.startswith(b'%PDF-') or type(page) is not int or page < 1:die('a PDF and a positive physical page number are required')
    try:result=subprocess.run(['pdftoppm','-f',str(page),'-l',str(page),'-singlefile','-r','150','-png'],input=raw,capture_output=True,check=True)
    except (OSError,subprocess.CalledProcessError) as error:die(f'PDF page rendering failed: {error}')
    if not valid_page_image(result.stdout):die('PDF page was not rendered; check its physical page number')
    return result.stdout

def cmd_pdf_page(args):
    path=packet(args.packet);data=witness_data(path,args.language) if args.language else evidence(path)
    if args.language and os.path.isfile(witness_file(path,args.language)+'.accepted'):die('an accepted witness cannot be changed')
    if not args.language and part(path,'accepted.json') is not None:die('an accepted packet cannot be changed')
    if not args.language and (not args.loc or args.loc not in (part(path,'places.json') or {}).get('places',{})):die('give an existing original place with --loc')
    source=next((s for s in data.get('sources',[]) if s.get('file')==args.source and not s.get('derived_from')),None)
    if not source or not lawful_url(source.get('url')):die('fetch the PDF source first')
    local=(lambda name:witness_cache(path,args.language,name)) if args.language else (lambda name:cache(path,name))
    raw=Path(local(source['file'])).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=source.get('sha256'):die('the fetched PDF has changed')
    independent=None
    if args.by=='second':
        raw=get(source['url'])
        if hashlib.sha256(raw).hexdigest()!=source['sha256']:die('the independently fetched PDF differs from the maker source')
        independent={'url':source['url'],'sha256':source['sha256'],'date':today()}
    image=render_pdf_page(raw,args.page);digest=hashlib.sha256(image).hexdigest()
    version=subprocess.run(['pdftoppm','-v'],capture_output=True,text=True).stderr.splitlines()[0]
    provenance={'file':source['file'],'sha256':source['sha256'],'tool':'pdftoppm','version':version,'page':args.page,'dpi':150}
    key=hashlib.sha256((source['url']+'\0'+source['file']).encode()).hexdigest()[:16]
    name=f'pdf-page-{key}-{source["sha256"]}-{args.page}-{digest}-{args.by}.png'
    Path(local(name)).write_bytes(image)
    record={'file':name,'url':source['url'],'sha256':digest,'by':args.by,'derived_from':provenance,**({'independent_fetch':independent} if independent else {})}
    if args.language:
        data['images']=[item for item in data.get('images',[]) if item.get('file')!=name]+[record]
        if args.by=='maker':data['review']={}
        save(witness_file(path,args.language),data)
    else:
        record.update({'item':'external','n':args.page,'loc':args.loc})
        scan={'item':'external','ocr':False,'image_offset':0};data['scans']=scans(data);data['scans']['external']=scan
        if not data.get('scan'):data['scan']=scan
        data['images']=[item for item in data.get('images',[]) if not(item.get('file')==name and item.get('loc')==args.loc)]+[record]
        if args.by=='maker':
            place=data.setdefault('places',{}).setdefault(args.loc,{});previous=dict(place);same=scan_of(data,place).get('item')=='external'
            place.update({'method':'image','scan':'external','images':sorted(set(place.get('images',[]) if same else [])|{args.page})})
            review=part(path,'review.json')
            if previous and previous!=place:data.setdefault('page_rebindings',[]).append({'loc':args.loc,'from':previous,'review':review,'date':today()})
            if review is not None:save(os.path.join(path,'review.json'),{})
        save(os.path.join(path,'evidence.json'),data)
    print(f'{local(name)}\nsha256 {digest}: read the rendered PDF page')

def derived_image_problems(sources,images,local,offline=False,legacy=False):
    problems=[]
    for image in images:
        derived=image.get('derived_from')
        if not derived:continue
        if not isinstance(derived,dict):problems.append('invalid rendered PDF page provenance');continue
        parent=next ( (s for s in sources if s.get('file')==derived.get('file') and s.get('sha256')==derived.get('sha256') and not s.get('derived_from')),None)
        if not parent or image.get('url')!=parent.get('url') or derived.get('tool')!='pdftoppm' or not derived.get('version') or type(derived.get('page')) is not int or derived['page']<1 or derived.get('dpi')!=150:
            problems.append('rendered PDF page needs its exact fetched predecessor and rendering operation');continue
        if image.get('by')=='second':
            proof=image.get('independent_fetch') or {}
            if not isinstance(proof,dict) or proof.get('url')!=parent.get('url') or proof.get('sha256')!=parent.get('sha256') or not proof.get('date'):
                (OWES if legacy else problems).append('second rendered PDF page needs an independent fetch of the same source bytes')
        if not offline:
            try:
                pdf=Path(local(parent['file'])).read_bytes();raw=Path(local(image['file'])).read_bytes()
                if not pdf.startswith(b'%PDF-') or hashlib.sha256(pdf).hexdigest()!=parent['sha256'] or hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):problems.append('rendered PDF page or its predecessor has changed')
            except (OSError,KeyError):problems.append('rendered PDF page or its predecessor is missing')
    return problems

def pdf_page_review_problems(ev,locs,looked):
    problems=[];images=ev.get('images') or []
    for loc in locs:
        entry=(ev.get('places') or {}).get(loc) or {}
        maker=[image for image in images if image.get('by')=='maker' and image.get('loc')==loc and (image.get('derived_from') or {}).get('tool')=='pdftoppm' and image.get('n') in (entry.get('images') or [])]
        for image in maker:
            same=lambda other: other.get('by')=='second' and other.get('loc')==loc and other.get('n')==image.get('n') and other.get('sha256')==image.get('sha256') and other.get('derived_from')==image.get('derived_from')
            reviewed=any(item.get('loc')==loc and item.get('n')==image.get('n') and item.get('reads') in ('as the packet','differs') for item in looked)
            if not any(same(other) for other in images) or not reviewed:problems.append(f'review.images: {loc} needs the same rendered PDF page {image.get("n")} and its second reading')
    return problems

def derived_source_problems(sources,local,offline=False,images=()):
    problems=[]
    for source in sources:
        derived=source.get('derived_from')
        if not derived:continue
        if not isinstance(derived,dict):problems.append('invalid derived PDF text provenance');continue
        if derived.get('tool')=='ocr-page-join':
            segments=derived.get('segments') or [];reason=derived.get('reason') or ''
            if not isinstance(segments,list) or len(segments)<2 or not isinstance(reason,str) or len(reason.strip())<40:problems.append('joined OCR needs ordered page segments and an omission assessment');continue
            chunks=[];parents=[]
            for segment in segments:
                if not isinstance(segment,dict):problems.append('invalid joined OCR segment');continue
                parent=next((s for s in sources if s.get('file')==segment.get('file') and s.get('sha256')==segment.get('sha256') and (s.get('derived_from') or {}).get('tool')=='tesseract-image'),None)
                if not parent or type(segment.get('start')) is not int or type(segment.get('end')) is not int or segment['start']<0 or segment['end']<=segment['start']:problems.append('joined OCR segment needs exact raw page and character offsets');continue
                parents.append(parent)
                if not offline:
                    try:
                        raw=Path(local(parent['file'])).read_bytes();content=raw.decode('utf-8')
                        if hashlib.sha256(raw).hexdigest()!=parent['sha256'] or segment['end']>len(content):raise ValueError()
                        chunks.append(content[segment['start']:segment['end']].strip())
                    except (OSError,KeyError,ValueError,UnicodeDecodeError):problems.append('joined OCR parent changed or is missing')
            if len(parents)!=len(segments) or source.get('url')!=(parents[0].get('url') if parents else None):problems.append('joined OCR must retain every page and its first source URL')
            if not offline and len(chunks)==len(segments):
                try:
                    output=Path(local(source['file'])).read_bytes()
                    if output!=' '.join(chunks).encode('utf-8') or hashlib.sha256(output).hexdigest()!=source.get('sha256'):problems.append('joined OCR differs from its recorded raw page segments')
                except (OSError,KeyError):problems.append('joined OCR is missing')
            continue
        if derived.get('tool')=='tesseract-image':
            image=next((i for i in images if i.get('file')==derived.get('file') and i.get('sha256')==derived.get('sha256') and i.get('by')=='maker'),None)
            if not image or source.get('url')!=image.get('url') or not derived.get('version') or not derived.get('leptonica') or not re.fullmatch(r'[a-zA-Z0-9_+-]+',str(derived.get('language',''))) or not re.fullmatch(r'[a-f0-9]{64}',str(derived.get('model_sha256',''))) or not re.fullmatch(r'[a-f0-9]{64}',str(derived.get('runner_sha256',''))) or derived.get('options')!=['default-page-segmentation','no-preprocessing']:
                problems.append('image OCR needs its exact maker page, model and operation');continue
            if not offline:
                try:
                    page=Path(local(image['file'])).read_bytes();text=Path(local(source['file'])).read_bytes();model=Path(local(derived['model_file'])).read_bytes();runner=Path(local(derived['runner_file'])).read_bytes()
                    if hashlib.sha256(page).hexdigest()!=image['sha256'] or not valid_page_image(page) or hashlib.sha256(text).hexdigest()!=source.get('sha256') or hashlib.sha256(model).hexdigest()!=derived['model_sha256'] or hashlib.sha256(runner).hexdigest()!=derived['runner_sha256']:problems.append('image OCR, page, model or runner changed')
                except (OSError,KeyError,TypeError):problems.append('image OCR, page, model or runner is missing')
            continue
        parent=next((item for item in sources if item.get('file')==derived.get('file') and item.get('sha256')==derived.get('sha256')),None)
        if not parent or source.get('url')!=parent.get('url') or derived.get('tool')!='pdftotext' or not derived.get('version') or derived.get('options')!=['-layout']:
            problems.append('derived PDF text needs its exact fetched predecessor and extraction operation');continue
        if not offline:
            try:
                pdf=Path(local(parent['file'])).read_bytes();text=Path(local(source['file'])).read_bytes()
                if not pdf.startswith(b'%PDF-') or hashlib.sha256(pdf).hexdigest()!=parent['sha256'] or hashlib.sha256(text).hexdigest()!=source.get('sha256'):problems.append('derived PDF text or its predecessor has changed')
            except (OSError,KeyError):problems.append('derived PDF text or its predecessor is missing')
    return problems

def scans(ev):
    """Every scan a packet reads (a work in several volumes has several), by its Archive item. ev['scan'] is the one the commands work on."""
    found=dict(ev.get('scans') or {});active=ev.get('scan') or {}
    if active.get('item'):found[active['item']]=active
    return found

def scan_of(ev,entry):
    """The scan a place stands on: the one its comparison found it in, or the one its words were cut from."""
    entry=entry or {};name=str(entry.get('transcription') or '')
    item=entry.get('scan') or (name[:-8] if entry.get('from_scan') and name.endswith('.ocr.txt') else None)
    return scans(ev).get(item) or ev.get('scan') or {}

def letters(text):
    """The letters of a word, with case, accents, ligatures, umlaut spellings and the long s set aside."""
    for a,b in (('ſ','s'),('æ','ae'),('Æ','ae'),('œ','oe'),('Œ','oe'),('ß','ss'),('ä','ae'),('ö','oe'),('ü','ue'),('Ä','ae'),('Ö','oe'),('Ü','ue')):text=text.replace(a,b)
    # Indic vowel signs and virama are part of the spelling, not accents.
    text=unicodedata.normalize('NFD',unicodedata.normalize('NFKC',text).casefold())
    return ''.join(c for c in text if c.isalpha() or ('\u0900'<=c<='\u097f' and unicodedata.category(c).startswith('M')))

def changed(a,b):
    """The words a correction changes in their letters, as pairs (before, after)."""
    wa,wb=PART.findall(a),PART.findall(b);na,nb=[letters(w) for w in wa],[letters(w) for w in wb]
    return [(' '.join(wa[i1:i2]),' '.join(wb[j1:j2])) for tag,i1,i2,j1,j2 in difflib.SequenceMatcher(None,na,nb,autojunk=False).get_opcodes() if tag!='equal' and ''.join(na[i1:i2])!=''.join(nb[j1:j2])]

def by_eye(places,ev):
    """The corrections that rest on a reader's eye alone: they change letters, and the page's OCR does not read them either
    (the place was read from the image, the OCR reads otherwise there, or the print sets the long s where the change is between s and f)."""
    found=[];work=places.get('work','')
    for correction in places.get('corrections') or []:
        pairs=changed(str(correction.get('from','')),str(correction.get('to','')))
        loc=str(correction.get('place','')).removeprefix(work+' ');entry=(ev.get('places') or {}).get(loc)
        if not pairs or entry is None:continue
        read={letters(word) for difference in entry.get('differences',[]) for word in PART.findall(difference.get('transcription',''))}
        deleted={letters(word) for difference in entry.get('differences',[]) for word in PART.findall(difference.get('ocr','')) if letters(word) not in {letters(w) for w in PART.findall(difference.get('transcription',''))}}
        long_s=scan_of(ev,entry).get('long_s')
        if entry.get('method')!='ocr' or any(letters(word) in read for _,after in pairs for word in PART.findall(after)) or any(not PART.findall(after) and any(letters(word) in deleted for word in PART.findall(before)) for before,after in pairs) or (long_s and any(letters(a).replace('f','s')==letters(b).replace('f','s') for a,b in pairs)):
            found.append((loc,'; '.join(f'«{a}» → «{b}»' for a,b in pairs)))
    return found

def enlarged(ev,by):
    """The places whose lines a reader has looked at enlarged."""
    return {image.get('loc') for image in ev.get('images',[]) if image.get('zoom') and image.get('by','maker')==by}

def shots(ev,by):
    """The page images fetched by a reader, by scan."""
    found={}
    for image in ev.get('images',[]):
        if image.get('by','maker')==by and not image.get('zoom'):found.setdefault(image.get('item') or image['file'][:-len(f"-n{image['n']}.jpg")],set()).add(image['n'])
    return found

def cmd_scan(args):
    """Which scans the packet reads; with ITEM, the scan the next commands work on."""
    path=packet(args.packet);ev=evidence(path);ev['scans']=scans(ev)
    if args.item:
        if args.item not in ev['scans']:die(f'{args.item} was not fetched (kit.py ia)')
        ev['scan']=ev['scans'][args.item];save(os.path.join(path,'evidence.json'),ev)
    for item,scan in ev['scans'].items():print(f"{'*' if item==(ev.get('scan') or {}).get('item') else ' '} {item}: OCR {'yes' if scan.get('ocr') else 'no'}, long s {'yes' if scan.get('long_s') else 'no'}, image offset {scan.get('image_offset')}")

def cmd_ia(args):
    path=packet(args.packet);item=args.item;wanted={'ocr':'_hocr_searchtext.txt.gz','index':'_hocr_pageindex.json.gz','pages':'_page_numbers.json'}
    if args.dir:names={role:os.path.join(args.dir,file) for role,file in zip(wanted,args.files.split(','))}
    else:
        metadata=json.loads(get(f'https://archive.org/metadata/{item}'))
        if (metadata.get('metadata') or {}).get('identifier')!=item:die(f'{item}: Archive metadata does not establish this item; no scan registered')
        files=[entry['name'] for entry in metadata.get('files',[])]
        names={role:next((name for name in files if name.endswith(tail)),None) for role,tail in wanted.items()}
    ev=evidence(path);was=scans(ev).get(item) or {}
    if not names['ocr'] or not names['index']:
        ev['scans']=scans(ev);ev['scan']=ev['scans'][item]={'item':item,'ocr':False,'long_s':bool(args.long_s or was.get('long_s')),'image_offset':was.get('image_offset')};save(os.path.join(path,'evidence.json'),ev)
        print(f'{item}: no page-indexed OCR; every place of this scan is read whole on the page image (judge … whole)');return
    for role,name in names.items():
        if not name:continue
        url=f'https://archive.org/download/{item}/{urllib.parse.quote(os.path.basename(name))}'
        raw=open(name,'rb').read() if args.dir else get(url)
        if name.endswith('.gz'):raw=gzip.decompress(raw)
        ev=record(path,'scan-'+role,url,f'{item}.{role}.'+('txt' if role=='ocr' else 'json'),raw)
    ev['scans']=scans(ev);ev['scan']=ev['scans'][item]={'item':item,'ocr':True,'long_s':bool(args.long_s or was.get('long_s')),'image_offset':was.get('image_offset')}
    save(os.path.join(path,'evidence.json'),ev);o=ocr(path,ev)
    if len(ev['scans'])>1:print(f"the packet reads {len(ev['scans'])} scans; this one is the scan the next commands work on (kit.py scan lists them and switches)")
    print(f"{item}: OCR of {len(o['starts'])} leaves, {len(o['words'])} words; printed page numbers {'given' if o['printed'] else 'not given'} by the Archive")

def cmd_scan_withdraw(args):
    path=packet(args.packet);ev=evidence(path);available=scans(ev)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot withdraw its scan')
    if args.item not in available:die('no such scan is registered')
    if len(args.reason.strip())<30:die('give a reason in a full sentence')
    if any(scan_of(ev,place).get('item')==args.item for place in (ev.get('places') or {}).values()):die('the scan is already used by a place; preserve the evidence and correct that place first')
    scan=available.pop(args.item)
    ev.setdefault('withdrawn_scans',[]).append({'scan':scan,'reason':args.reason,'date':today()})
    ev['scans']=available
    if (ev.get('scan') or {}).get('item')==args.item:ev['scan']={}
    save(os.path.join(path,'evidence.json'),ev)
    print(f'{args.item}: withdrawn; prior record and sources preserved')

def cmd_scan_rebind(args):
    path=packet(args.packet);ev=evidence(path);places=part(path,'places.json') or {};entry=(ev.get('places') or {}).get(args.loc);place=(places.get('places') or {}).get(args.loc);scan=scans(ev).get(args.item)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its scan binding')
    if not entry or not place or not scan or not scan.get('ocr') or type(scan.get('image_offset')) is not int or len(args.reason.strip())<40:die('choose an existing place, registered OCR scan with offset, and detailed reason')
    if entry.get('scan')==args.item:die('the place already names that scan')
    if entry.get('from_scan') and entry.get('transcription')!=f'{args.item}.ocr.txt':die('the prior transcription is another scan’s OCR; preserve it and establish a source for this edition first')
    image=next((i for i in ev.get('images') or [] if i.get('file')==args.image and i.get('loc')==args.loc and i.get('by','maker')=='maker'),None)
    url=urllib.parse.urlsplit(str((image or {}).get('url') or ''));match=re.fullmatch(r'/download/'+re.escape(args.item)+r'/page/n(\d+)(?:_w\d+)?\.(?:jpg|jpeg|png)',url.path,re.I)
    if not image or url.scheme!='https' or url.hostname not in ('archive.org','www.archive.org') or not match:die('the maker image must be the exact Archive page URL for this registered scan')
    try:
        raw=Path(cache(path,image['file'])).read_bytes()
        if hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):die('the existing maker image changed')
        if image.get('transform'):
            original=Path(cache(path,image['original_file'])).read_bytes()
            if hashlib.sha256(original).hexdigest()!=image.get('original_sha256') or not valid_page_image(original):die('the original scan image changed')
        for role,suffix in (('scan-ocr','ocr.txt'),('scan-index','index.json')):
            filename=f'{args.item}.{suffix}';source=next((s for s in ev.get('sources') or [] if s.get('role')==role and s.get('file')==filename),None)
            if not source or hashlib.sha256(Path(cache(path,filename)).read_bytes()).hexdigest()!=source.get('sha256'):die('the registered scan OCR or index changed')
        original=original_of(places);text=(place.get('texts') or {}).get(original);found=against_ocr(text,ocr(path,ev,scan),modern_ru=original=='ru',script='zh' if original.startswith('zh') else original if original in ('la','sa') else None)
    except (OSError,KeyError,ValueError,TypeError):die('the scan, its OCR or the original place is unavailable')
    n=int(match.group(1))
    if not found or not found.get('leaves') or n not in {leaf+scan['image_offset'] for leaf in found['leaves']}:die('the OCR and registered page offset do not locate this place on the selected image')
    prior_review=part(path,'review.json');ev.setdefault('scan_rebindings',[]).append({'loc':args.loc,'from':entry,'to':args.item,'image':image,'reason':args.reason,'review':prior_review,'date':today()})
    preserved={k:entry[k] for k in ('transcription','dropped','from_scan','quotation') if k in entry};preserved.update({'scan':args.item,'scan_rebind_pending':True})
    # Other legacy places may rely on the active scan as their implicit default.
    ev['places'][args.loc]=preserved;ev['scans']=scans(ev)
    rebound={**image,'item':args.item,'n':n,'rebound_from':{'file':image['file'],'sha256':image['sha256'],'url':image['url']}}
    ev.setdefault('images',[]).append(rebound);save(os.path.join(path,'evidence.json'),ev)
    if prior_review is not None:save(os.path.join(path,'review.json'),{})
    print('scan rebound with prior evidence and review retained; run compare and obtain a fresh independent reading')

# ---- words ---------------------------------------------------------------------------------------------------------

QUOTES=str.maketrans({'“':'"','”':'"','„':'"','«':'"','»':'"','‟':'"','‘':"'",'’':"'",'‚':"'",'‛':"'",'—':'-','–':'-','‒':'-','‐':'-','‑':'-','⸗':'-','…':'.'})
_LETTER=r'(?:(?![\u3400-\u9fff\u3040-\u30ff])[^\W\d_]|[\u0900-\u0903\u093a-\u094f\u0951-\u0957\u0962-\u0963])'
TOKEN=re.compile(r"[\u3400-\u9fff\u3040-\u30ff]|"+_LETTER+r"+(?:'"+_LETTER+r"+)*|\d+|[^\s\w]|_")
WORD=re.compile(r'[\u3400-\u9fff\u3040-\u30ff]+|'+_LETTER+r'+')
PART=re.compile(r'[\u3400-\u9fff\u3040-\u30ff]|'+_LETTER+r'+|\d+')

# IAST is a search key only; the printed and transcribed words stay in the evidence.
_VOWELS=dict(zip('अआइईउऊऋॠऌॡएऐओऔ',('a','ā','i','ī','u','ū','ṛ','ṝ','ḷ','ḹ','e','ai','o','au')))
_SIGNS=dict(zip('ािीुूृॄॢॣेैोौ',('ā','i','ī','u','ū','ṛ','ṝ','ḷ','ḹ','e','ai','o','au')))
_CONSONANTS=dict(zip('कखगघङचछजझञटठडढणतथदधनपफबभमयरलवशषसहळ',('k','kh','g','gh','ṅ','c','ch','j','jh','ñ','ṭ','ṭh','ḍ','ḍh','ṇ','t','th','d','dh','n','p','ph','b','bh','m','y','r','l','v','ś','ṣ','s','h','ḻ')))
_SANSKRIT_MARKS={'ं':'ṃ','ः':'ḥ','ँ':'m̐','ऽ':'’'}
# A small, explicit set of frequent traditional/simplified or graphic variants.
# Unknown variants remain visible mismatches; a folded match still requires page review.
_HAN_VARIANTS=str.maketrans({'國':'国','學':'学','體':'体','藝':'艺','樂':'乐','聲':'声','書':'书','畫':'画','萬':'万','與':'与','爲':'为','為':'为','說':'说','時':'时','見':'见','無':'无','有':'有','異':'异','風':'风','氣':'气'})
def iast(text):
    """Transliterate Devanagari Sanskrit for lookup, keeping unrecognised signs visible."""
    text=unicodedata.normalize('NFC',text);out=[];i=0
    while i<len(text):
        c=text[i]
        if c in _CONSONANTS:
            out.append(_CONSONANTS[c]);i+=1
            if i<len(text) and text[i]=='्':i+=1
            elif i<len(text) and text[i] in _SIGNS:out.append(_SIGNS[text[i]]);i+=1
            else:out.append('a')
            continue
        out.append(_VOWELS.get(c,_SANSKRIT_MARKS.get(c,c)));i+=1
    return unicodedata.normalize('NFC',''.join(out))

def key(token,long_s=False,script=None):
    token=unicodedata.normalize('NFKC',token).casefold().replace("'",'')
    if script=='sa':token=iast(token)
    if script=='la':token=token.translate(str.maketrans({'j':'i','v':'u'}))
    if script=='zh':token=token.translate(_HAN_VARIANTS)
    return token.replace('f','s') if long_s else token
def tokens(text,broken=False):
    """Words, numbers and marks with their offsets. In an OCR text (broken) a word divided at a line end is joined."""
    found=[(m.group(0),m.start()) for m in TOKEN.finditer(text.translate(QUOTES))];out=[];k=0
    while k<len(found):
        token,at=found[k]
        if broken and k+2<len(found) and text[found[k+1][1]] in '-¬=⸗' and found[k+1][1]==at+len(token) and found[k+2][1]>found[k+1][1]+1 and token[-1:].isalpha() and found[k+2][0][:1].islower():
            out.append((token+found[k+2][0],at));k+=3
        else:out.append((token,at));k+=1
    return out

def ocr(path,ev,scan=None):
    scan=scan or ev.get('scan') or {}
    if not scan.get('ocr'):return None
    item=scan['item'];raw=open(cache(path,f'{item}.ocr.txt'),encoding='utf-8',errors='replace').read()
    starts=[entry[0] for entry in load(cache(path,f'{item}.index.json'))]
    pages=cache(path,f'{item}.pages.json')
    printed={entry['leafNum']:entry.get('pageNumber') for entry in load(pages).get('pages',[]) if entry.get('pageNumber') not in(None,'')} if os.path.exists(pages) else {}
    found=tokens(raw,broken=True);return {'raw':raw,'starts':starts,'printed':printed,'words':[token for token,_ in found],'at':[at for _,at in found],'long_s':bool(scan.get('long_s'))}

def locate(needle,hay,index):
    """Where a run of keys stands in a longer run: the start that matches best among those its rarer words point to."""
    votes={}
    for offset,word in enumerate(needle[:40]):
        places=index.get(word,())
        cjk=len(word)==1 and ('\u3400'<=word<='\u9fff' or '\u3040'<=word<='\u30ff')
        if (len(word)<4 and not cjk) or not places or len(places)>60:continue
        for place in places:votes[place-offset]=votes.get(place-offset,0)+1
    if not votes:
        for offset,word in enumerate(needle[:12]):
            for place in index.get(word,())[:2000]:votes[place-offset]=votes.get(place-offset,0)+1
    best=None
    for start,_ in sorted(votes.items(),key=lambda vote:-vote[1])[:8]:
        start=max(0,start-15);window=hay[start:start+len(needle)+60]           # a note or a figure mark inside the run shifts the votes: look a little before them
        match=difflib.SequenceMatcher(None,needle,window,autojunk=False);matched=sum(block.size for block in match.get_matching_blocks())
        if best is None or matched>best[0]:best=(matched,start,match)
    return best

def against_ocr(text,o,modern_ru=False,script=None):
    """A place beside the OCR: its leaves, how much of it the OCR reads the same, and every difference."""
    mine=tokens(text);a=[token for token,_ in mine]
    normalized=modern_russian('\n'.join(a)).split('\n') if modern_ru else a
    ak=[key(token,o['long_s'],script) for token in normalized]
    cache_key=('modern_keys' if modern_ru else 'keys')+'_'+str(script);index_key=cache_key+'_index'
    if cache_key not in o:
        words=modern_russian('\n'.join(o['words'])).split('\n') if modern_ru else o['words']
        o[cache_key]=[key(token,o['long_s'],script) for token in words];o[index_key]={}
        for position,word in enumerate(o[cache_key]):o[index_key].setdefault(word,[]).append(position)
    best=locate(ak,o[cache_key],o[index_key])
    if not best:return None
    matched,start,match=best;blocks=[block for block in match.get_matching_blocks() if block.size]
    if not blocks or matched/len(ak)<0.75:return {'share':round(matched/len(ak),3) if ak else 0}
    leaf=lambda position:bisect.bisect_right(o['starts'],o['at'][min(start+position,len(o['at'])-1)])-1
    ops=match.get_opcodes();differences=[]
    for n,(tag,i1,i2,j1,j2) in enumerate(ops):
        if tag=='equal':
            # Search folding must not hide a different printed spelling or script from review.
            if script:
                for i,j in zip(range(i1,i2),range(j1,j2)):
                    if key(a[i],o['long_s'])!=key(o['words'][start+j],o['long_s']):
                        differences.append({'transcription':a[i],'ocr':o['words'][start+j],'leaf':leaf(j)})
            continue
        if tag=='insert' and n in(0,len(ops)-1):continue
        differences.append({'transcription':' '.join(a[i1:i2]),'ocr':' '.join(o['words'][start+j1:start+j2]),'leaf':leaf(j1)})
    first,last=leaf(blocks[0].b),leaf(blocks[-1].b+blocks[-1].size-1)
    return {'share':round(matched/len(ak),3),'leaves':list(range(first,last+1)),'printed':[o['printed'].get(n) for n in range(first,last+1)],'differences':differences}

def in_transcription(text,source,dropped=None):
    """Whether a place is a run of the transcription, letter for letter as kit.py cut writes it, and opens a sentence there."""
    # Unspaced CJK text cannot be split into words by the Latin OCR tokenizer.
    # Accept an exact run only at a source sentence boundary; page review still applies.
    if re.search(r'[\u3400-\u9fff\u3040-\u30ff]',text) and not dropped:
        positions=[m.start() for m in re.finditer(re.escape(text),source)]
        for position in positions:
            before=source[:position].rstrip().rstrip(CLOSING).rstrip()
            if not before or before[-1] in '.!?:…。！？؟।॥':return None
        if positions:return 'it opens inside a sentence of the transcription'
    words=[(m.group(0),m.start()) for m in WORD.finditer(source.translate(QUOTES))];hay=[key(word) for word,_ in words];index={}
    for position,word in enumerate(hay):index.setdefault(word,[]).append(position)
    needle=[key(m.group(0)) for m in WORD.finditer(text.translate(QUOTES))];best=locate(needle,hay,index)
    if not best:return 'its words are not found in the transcription'
    matched,start,match=best;ops=match.get_opcodes();extra=0
    for n,(tag,i1,i2,j1,j2) in enumerate(ops):
        if tag in('replace','delete'):return f"the transcription reads «{' '.join(word for word,_ in words[start+j1:start+j2]) or '∅'}» where the place reads «{' '.join(needle[i1:i2])}»"
        if tag=='insert' and n not in(0,len(ops)-1):extra+=j2-j1
    if extra>max(6,len(needle)*0.03):return f'the transcription holds {extra} words inside the run that the place leaves out'
    first=next(block for block in match.get_matching_blocks() if block.size)
    before=source[max(0,words[start+first.b][1]-240):words[start+first.b][1]]
    opened=bool(re.search(r'\n\s*\n[\s"“„«‘\'(\[_*¿¡]*$',before)) or not before.strip()
    before=re.sub(r'(\[[^\]]{0,12}\]|\(\d+\)|\d{1,4}\)|[⁰¹²³⁴⁵⁶⁷⁸⁹]+\)?|[\d*†‡§]+)\s*$','',re.sub(r'[\s"“„«‘\'(\[_*¿¡]+$','',before))
    before=re.sub(r'[\s"”»’\')\]_*]+$','',re.sub(r'[\s—–-]+$','',re.sub(r'[\s"”»’\')\]_*]+$','',before)))   # «… kommen. — Eben jene …»: the dash after a full stop opens a sentence
    if not opened and before and before[-1] not in '.!?:…。！？؟।॥' and not (before[-1]==';' and re.search(r'[\u0370-\u03ff\u1f00-\u1fff]',text)):return f'it opens inside a sentence of the transcription (after «…{before[-40:]}»)'
    blocks=[block for block in match.get_matching_blocks() if block.size];a=words[start+blocks[0].b][1];last=words[start+blocks[-1].b+blocks[-1].size-1];z=last[1]+len(last[0])
    def written(end,begin=a):
        run=source[begin:end]
        for pattern in dropped or []:run=re.sub(pattern,' ',run)
        return re.sub(r' +([,;:.!?])',r'\1',squeeze(run)) if dropped else squeeze(run)
    # A sentence of the place may open with the mark of speech or a bracket the transcription sets before its first word.
    begins=[a]+[a-n for n in range(1,4) if a-n>=0 and all(mark in '"“„«‚‘\'([¿¡' for mark in source[a-n:a])]
    if any(written(z+more,begin)==text for begin in begins for more in range(0,60)):return None
    run=written(z+len(text)-len(text.rstrip('.!?…"”»’\')')));at=next((i for i,(x,y) in enumerate(zip(text,run)) if x!=y),min(len(text),len(run)))
    return f'its text is not the transcription’s as kit.py cut writes it: the place has «{text[max(0,at-25):at+25]}», the file «{run[max(0,at-25):at+25]}» (cut it again; --drop leaves out a transcriber’s mark)'

def squeeze(text):
    """The words of a run as a place keeps them: one space, paragraphs as blank lines, emphasis marks and note anchors left out."""
    text=re.sub(r'\[(?:\d{1,3}|[A-Z])\]|(?<=\w)\*+|\*+(?=\w)','',text)
    text=re.sub(r'(?<!\w)_|_(?!\w)','',text)
    return unicodedata.normalize('NFC','\n\n'.join(' '.join(chunk.split()) for chunk in re.split(r'\n\s*\n',text) if chunk.strip()))

def page_text(raw):
    """The OCR of a scan as running text: words divided at line ends joined, ſ written s."""
    return re.sub(r'(?<=[^\W\d_])[-¬⸗]\s+(?=[a-zß-ÿ])','',raw.replace('ſ','s'))

def transcription(path,ev,name=None):
    if name and name.endswith('.ocr.txt') and any(source['file']==name and source['role']=='scan-ocr' for source in ev['sources']):
        return next(source for source in ev['sources'] if source['file']==name),page_text(open(cache(path,name),encoding='utf-8',errors='replace').read())
    sources=[source for source in ev['sources'] if source['role']=='transcription' and (name is None or name in(source['file'],source.get('text')))]
    if not sources:die('no fetched transcription'+(f' named {name}' if name else ''))
    return sources[0],Path(cache(path,sources[0]['text'])).read_text(encoding='utf-8')

def cmd_cut(args):
    path=packet(args.packet);ev=evidence(path)
    if args.file=='ocr':
        scan=ev.get('scan') or {}
        if not scan.get('ocr'):die('this packet has no page-indexed OCR to cut from (kit.py ia)')
        args.file=f"{scan['item']}.ocr.txt"
    source,text=transcription(path,ev,args.file)
    words=lambda phrase:re.compile(r'\s+'.join(re.escape(word) for word in phrase.split()))
    opens=list(words(args.open).finditer(text))
    if not opens:die(f'the opening words are not in {source.get("text",source["file"])} (give them exactly as the file has them; kit.py find … --in helps)')
    if len(opens)>1 and not args.nth:print(f'note: the opening words stand {len(opens)} times; the first is taken (--nth N takes another)')
    if (args.nth or 1)>len(opens):die(f'the opening words stand {len(opens)} times only')
    start=opens[(args.nth or 1)-1].start();close=words(args.close).search(text,start)
    if not close:die('the closing words are not found after the opening words')
    run=text[start:close.end()]
    for pattern in args.drop or []:run=re.sub(pattern,' ',run)
    run=re.sub(r' +([,;:.!?])',r'\1',squeeze(run)) if args.drop else squeeze(run);places=part(path,'places.json');original=(places.get('entry') or {}).get('original') or world()['fragments']['works'].get(places['work'],{}).get('original')
    lang=args.lang or original
    if not lang:die('places.json names no original language yet (entry.original)')
    normalization=None
    if lang=='ru':
        modern=modern_russian(run,old_source=old_russian_source(text))
        if modern!=run:
            witness=((places.get('entry') or {}).get('witnesses') or {}).get(lang) or ((world()['fragments']['works'].get(places['work']) or {}).get('witnesses') or {}).get(lang)
            if witness is None:die('Russian modernization requires the original witness')
            note='Russian spelling modernized using the 1918 reform rules; ѵ rendered as и; wording unchanged.'
            normalization={'source_text':run,'rule':note}
            if places.get('entry'):
                rights=witness.setdefault('rights',{});reason=rights.get('reasoning','')
                if note not in reason:rights['reasoning']=(reason+' '+note).strip()
            run=modern
    place=places['places'].setdefault(args.loc,{'label':{'ru':'','en':''},'texts':{},'links':{},'refs':[]});place['texts'][lang]=run
    if normalization:place.setdefault('normalization',{})[lang]=normalization
    elif place.get('normalization'):place['normalization'].pop(lang,None)
    save(os.path.join(path,'places.json'),places);entry=ev['places'].setdefault(args.loc,{});entry['transcription']=source['file'];entry.pop('dropped',None);entry.pop('from_scan',None)
    if source['role']=='scan-ocr':entry['from_scan']=True;print('cut from the scan’s own OCR: there is no second text behind this place, so it is read whole on the page image (image N --loc, correct, judge … whole)')
    if args.drop:entry['dropped']=args.drop
    save(os.path.join(path,'evidence.json'),ev)
    print(f'{args.loc} [{lang}] {len(run)} characters:\n{run}')
    for problem in sentences(run):print('  ! '+problem)

OPENING='"“„«‚‘「『\'([—–- ¿¡';CLOSING='"”“»’‘」』\')] '        # the marks of speech and brackets a sentence may stand in: a quoted sentence is a whole sentence

def sentences(text,limit=LONGEST,language=None):
    problems=[]
    if text!=unicodedata.normalize('NFC',text):problems.append('the text is not NFC')
    if text!=text.strip() or '  ' in text or '\t' in text:problems.append('stray whitespace')
    first=text.lstrip(OPENING)[:1]
    greek_initial=bool(first and re.fullmatch(r'[\u0370-\u03ff\u1f00-\u1fff]',first))
    latin_initial=language=='la' and first.isalpha()       # Latin editions open a sentence in lower case; the transcription boundary is checked apart
    if not first or not first.isalpha() or (first.lower()!=first.upper() and not first.isupper() and not greek_initial and not latin_initial):problems.append(f'it does not open with a capital letter or uncased sentence initial («{text[:24]}»)')
    ending=text.rstrip(CLOSING)[-1:]
    greek_question=ending==';' and re.search(r'[\u0370-\u03ff\u1f00-\u1fff]',text)
    if ending not in '.!?…。！？؟।॥' and not greek_question:problems.append(f'it does not close a sentence («{text[-24:]}»)')
    if limit is not None and len(text)>limit:problems.append(f'{len(text)} characters: a place is the sentences that say the thing, at most {limit} characters')
    return problems

def cmd_find(args):
    path=packet(args.packet);ev=evidence(path)
    if args.inside:
        _,text=transcription(path,ev,args.inside);flat=re.compile(r'\s+'.join(re.escape(word) for word in args.words.split()),re.I)
        for m in list(flat.finditer(text))[:12]:print(f'@{m.start()}: …{" ".join(text[max(0,m.start()-160):m.end()+260].split())}…\n')
        return
    places=part(path,'places.json',{});original=(places.get('entry') or {}).get('original') or (world()['fragments']['works'].get(places.get('work')) or {}).get('original')
    script='zh' if (original or '').startswith('zh') else original if original in ('la','sa') else None
    found=False
    for item,scan in scans(ev).items():
        o=ocr(path,ev,scan);got=against_ocr(args.words,o,script=script) if o else None
        if got and 'leaves' in got:found=True;print(f"{item}: leaves {got['leaves']} (printed pages {got['printed']}), {round(got['share']*100)}% read the same")
    if not found:die('the words are not found in the OCR of any scan of this packet')

def cmd_leaf(args):
    path=packet(args.packet);o=ocr(path,evidence(path)) or die('no page-indexed OCR in this packet');n=args.n
    print(f"leaf {n}, printed page {o['printed'].get(n)}:\n"+o['raw'][o['starts'][n]:o['starts'][n+1] if n+1<len(o['starts']) else None])

def band(file,top,bottom,width=1600):
    """The band of a page between two percents of its height, at the size of the scan: where one letter is in question, the whole page is too small to read it."""
    try:
        from PIL import Image
        with Image.open(file) as image:
            part_=image.convert('L').crop((0,round(image.height*top/100),image.width,round(image.height*bottom/100)))
            part_.resize((width,max(1,round(part_.height*width/part_.width)))).save(file,quality=85)
    except ImportError:
        tool=next((name for name in('magick','convert') if subprocess.run(['which',name],capture_output=True).returncode==0),None) or die('the enlarged line wants Pillow or ImageMagick')
        w,h=(int(n) for n in subprocess.run([tool,file,'-format','%w %h','info:'],capture_output=True,text=True).stdout.split()[:2])
        subprocess.run([tool,file,'-colorspace','Gray','-crop',f'{w}x{round(h*(bottom-top)/100)}+0+{round(h*top/100)}','+repage','-resize',f'{width}x','-quality','85',file],check=True)
    return Path(file).read_bytes()

def narrow(file,width):
    """The Archive answers with the nearest size it keeps, often twice the width asked for: bring the page to the width asked, so that a reader's look costs what a page costs."""
    try:
        from PIL import Image
        with Image.open(file) as image:
            if image.width>width*1.15:image.convert('L').resize((width,round(image.height*width/image.width))).save(file,quality=82)
    except ImportError:
        for tool in('magick','convert'):
            try:
                if subprocess.run([tool,file,'-resize',f'{width}x>','-colorspace','Gray','-quality','82',file],capture_output=True).returncode==0:break
            except FileNotFoundError:continue
    return Path(file).read_bytes()

def preserve_scan_source(path,raw):
    if not valid_page_image(raw):die('the downloaded scan page is not a readable image')
    digest=hashlib.sha256(raw).hexdigest();name=f'scan-source-{digest}.img';destination=Path(cache(path,name))
    if destination.exists() and destination.read_bytes()!=raw:die('source image filename already holds different bytes; preserve it')
    if not destination.exists():destination.write_bytes(raw)
    return {'original_file':name,'original_sha256':digest}

def cmd_image(args):
    path=packet(args.packet);ev=evidence(path)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its page evidence')
    item=args.scan or (scan_of(ev,ev['places'].get(args.loc)).get('item') if args.loc and ev['places'].get(args.loc) else None) or (ev.get('scan') or {}).get('item') or die('name the scan first (kit.py ia)')
    if item not in scans(ev):die(f'{item} is not a scan of this packet (kit.py ia)')
    if args.zoom:
        try:top,bottom=(int(part_) for part_ in args.zoom.split('-'));assert 0<=top<bottom<=100 and bottom-top<=25
        except (ValueError,AssertionError):die('--zoom wants the band of the page height in percent that holds the lines, as 41-48 (25 at most)')
        if not args.loc:die('--zoom wants --loc: the place whose line is enlarged')
        url=f'https://archive.org/download/{item}/page/n{args.n}.jpg';source=Path(args.file).read_bytes() if args.file else get(url)
        fd,temp=tempfile.mkstemp(prefix='image-',suffix='.jpg',dir=os.path.dirname(cache(path,'placeholder')))
        try:
            with os.fdopen(fd,'wb') as file:file.write(source)
            raw=band(temp,top,bottom)
        finally:os.unlink(temp)
        if not valid_page_image(raw):die('the scan did not yield a readable enlarged image')
        original=preserve_scan_source(path,source)
        name=f'{item}-n{args.n}-z{top}-{bottom}-{args.by}-{hashlib.sha256(raw).hexdigest()}.jpg'
        destination=Path(cache(path,name))
        if destination.exists() and destination.read_bytes()!=raw:die('image filename already holds different bytes; preserve it')
        if not destination.exists():destination.write_bytes(raw)
        ev['images']=[image for image in ev.get('images',[]) if not(image['file']==name and image.get('by')==args.by)]
        ev['images'].append({'n':args.n,'item':item,'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'by':args.by,'loc':args.loc,'url':url,**original,'transform':{'method':'band','top':top,'bottom':bottom},'zoom':[top,bottom]})
        save(os.path.join(path,'evidence.json'),ev);print(f'{cache(path,name)}\nlook at this file: the lines between {top}% and {bottom}% of the page, at the size of the scan. Where the line sits elsewhere, ask again with another band');return
    url=f'https://archive.org/download/{item}/page/n{args.n}_w{args.width}.jpg';source=Path(args.file).read_bytes() if args.file else get(url)
    fd,temp=tempfile.mkstemp(prefix='image-',suffix='.jpg',dir=os.path.dirname(cache(path,'placeholder')))
    try:
        with os.fdopen(fd,'wb') as file:file.write(source)
        raw=narrow(temp,args.width)
    finally:os.unlink(temp)
    if not valid_page_image(raw):die('the scan did not yield a readable page image')
    original=preserve_scan_source(path,source)
    name=f'{item}-n{args.n}-{args.by}-{hashlib.sha256(raw).hexdigest()}.jpg';destination=Path(cache(path,name))
    if destination.exists() and destination.read_bytes()!=raw:die('image filename already holds different bytes; preserve it')
    if not destination.exists():destination.write_bytes(raw)
    ev['images']=[image for image in ev.get('images',[]) if not(image['file']==name and image.get('by')==args.by)]
    ev['images'].append({'n':args.n,'item':item,'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'by':args.by,'url':url,**original,'transform':{'method':'narrow','width':args.width},**({'loc':args.loc} if args.loc else {})})
    if args.loc and args.by=='maker':
        entry=ev['places'].setdefault(args.loc,{})
        bound=scan_of(ev,entry).get('item') if entry else item
        if entry.get('method')!='ocr' and bound==item:entry['method']='image';entry['images']=sorted(set(entry.get('images',[]))|{args.n});entry.setdefault('scan',item)
    save(os.path.join(path,'evidence.json'),ev);print(f'{cache(path,name)}\nlook at this file; the page of the place opens at https://archive.org/details/{item}/page/n{args.n}/mode/1up')

TRUSTED=('wikipedia.org','wikidata.org','d-nb.info','viaf.org','bnf.fr','plato.stanford.edu','britannica.com','deutsche-biographie.de','loc.gov','oxforddnb.com','treccani.it','rusneb.ru','rsl.ru','nlr.ru','bne.es','eha.ee','ci.nii.ac.jp','cardiffnaturalists.org.uk')
def trusted(url):
    host=urllib.parse.urlparse(str(url)).netloc.lower();return str(url).startswith('https://') and any(host==name or host.endswith('.'+name) for name in TRUSTED)

def same_participant(expected,actual):
    if expected.get('qid') and actual.get('qid'):return expected['qid']==actual['qid']
    left=expected.get('name') or {};right=actual.get('name') or {}
    normalize=lambda value:unicodedata.normalize('NFKC',str(value)).casefold().strip()
    return isinstance(left,dict) and isinstance(right,dict) and bool({normalize(value) for value in left.values() if value}&{normalize(value) for value in right.values() if value})

def layer_authors(w,work):
    return [{'role':'author','name':w['agents'][agent]['name'],**({'qid':w['lives'][agent]['item']} if (w.get('lives',{}).get(agent) or {}).get('item') else {})}
            for agent in w.get('works',{}).get(work,{}).get('agents',[]) if agent in w.get('agents',{})]

def entity_year(entity,property_):
    claims=[claim for claim in entity.get('claims',{}).get(property_,[]) if claim.get('rank')!='deprecated' and claim.get('mainsnak',{}).get('datavalue')]
    preferred=[claim for claim in claims if claim.get('rank')=='preferred']
    chosen=preferred or claims
    values=[claim['mainsnak']['datavalue']['value'] for claim in chosen]
    # Calendar annotations are not uncertainty, except near the year boundary.
    calendars={'Q38173183','Q26877143','Q26877139','Q27055388'}
    if not values:return None
    for claim,value in zip(chosen,values):
        model=value.get('calendarmodel')
        if model and model!='http://www.wikidata.org/entity/Q1985727':
            # A Julian year can cross the Gregorian boundary. Other calendars
            # need an explicit conversion before they establish a legal year.
            date=value.get('time','')
            middle=re.match(r'^\+(?:0\d{3}|1\d{3})-(?:0[2-9]|1[01])-\d{2}T',date) and int(date[1:5])>0
            recent=re.match(r'^\+1[89]\d\d-',date) and not re.search(r'-12-(?:1[89]|2\d|3[01])T',date)
            if model!='http://www.wikidata.org/entity/Q1985786' or value.get('precision',0)<11 or not (middle or recent):return None
        all_qualifiers=claim.get('qualifiers') or {}
        if any(all_qualifiers.get(prop) for prop in ('P1319','P1326')):return None
        qualifiers=all_qualifiers.get('P1480') or []
        ids=[((item.get('datavalue') or {}).get('value') or {}).get('id') for item in qualifiers]
        if any(qid not in calendars for qid in ids):return None
        if qualifiers and (value.get('precision',0)==9 or re.search(r'-(?:00-00|12-(?:00|1[89]|2\d|3[01]))T',value.get('time',''))):return None
        if value.get('precision',0)<9 or value.get('before',0) or value.get('after',0):return None
    years={int(value['time'][:5]) for value in values}
    return next(iter(years)) if len(years)==1 else None

def death_bound(entity):
    """A conservative upper bound of a death from Wikidata: the latest year a claim of death (P570) allows by its
    precision, its «circa» qualifier and its latest-date qualifier. Without a claim of death, a floruit (P1317) or an
    end of working period (P2032) shows the person alive in its latest year, so the bound is that year plus the
    longest attested human life. The returned year is exclusive: one year after the latest possible year.
    None when no claim bounds the death before BOUND_BEFORE; an exact year is never
    invented from it."""
    spans={11:0,10:0,9:0,8:10,7:100,6:1000}
    def year_of(value):
        m=re.match(r'^([+-])(\d+)-',str((value or {}).get('time','')))
        return None if not m else int(m.group(2))*(-1 if m.group(1)=='-' else 1)
    units={11:1/365,10:1/12,9:1,8:10,7:100,6:1000}       # Wikibase counts «after» in units of the value's precision
    def upper_of(value):                              # the last year a time value allows by its precision and its «after»
        y=year_of(value);precision=(value or {}).get('precision')
        if y is None or precision not in spans:return None
        calendar=(value or {}).get('calendarmodel')
        if calendar and calendar not in ('http://www.wikidata.org/entity/Q1985727','http://www.wikidata.org/entity/Q1985786'):return None
        julian=str(calendar or '').endswith('/Q1985786')   # the end of a Julian interval may fall in the next Gregorian year
        return y+spans[precision]+math.ceil(units[precision]*int((value or {}).get('after') or 0))+(1 if julian else 0)
    death=[c for c in entity.get('claims',{}).get('P570',[]) if c.get('rank')!='deprecated']
    claims=[(prop,c) for prop in ('P570','P1317','P2032') for c in entity.get('claims',{}).get(prop,[]) if c.get('rank')!='deprecated' and (prop=='P570' or not death)]
    uppers=[];used=[]
    for prop,claim in claims:
        value=((claim.get('mainsnak') or {}).get('datavalue') or {}).get('value') or {}
        y=year_of(value);precision=value.get('precision')
        if y is None or precision not in spans:return None
        qualifiers=claim.get('qualifiers') or {}
        latest_values=[((q.get('datavalue') or {}).get('value')) or {} for q in qualifiers.get('P1326') or []]
        latest=[upper_of(v) for v in latest_values]
        if any(v is None for v in latest):return None   # a latest date of unknown precision bounds nothing
        circa=any(((q.get('datavalue') or {}).get('value') or {}).get('id')=='Q5727902' for q in qualifiers.get('P1480') or [])
        earliest=[year_of(((q.get('datavalue') or {}).get('value'))) for q in qualifiers.get('P1319') or []]
        if any(v is not None for v in earliest) and not any(v is not None for v in latest):return None   # «not before» alone leaves the death open
        if any(((q.get('datavalue') or {}).get('value') or {}).get('id')!='Q5727902' for q in qualifiers.get('P1480') or []):return None   # presumed or disputed: a reference decides
        base=upper_of(value)
        if base is None:return None
        upper=min(v for v in latest if v is not None) if any(v is not None for v in latest) else base+(LONGEST_LIFE+1 if circa else 0)   # allow the partial 123rd calendar year
        if prop!='P570':upper+=LONGEST_LIFE+1         # an observation late in a year can reach the following calendar year
        uppers.append(upper);used.append({'property':prop,'time':value.get('time'),'precision':precision,'calendar':value.get('calendarmodel'),'after':int(value.get('after') or 0),'circa':circa,'latest':[{'time':v.get('time'),'precision':v.get('precision'),'calendar':v.get('calendarmodel'),'after':int(v.get('after') or 0),'upper':u} for v,u in zip(latest_values,latest)],'earliest':[v for v in earliest if v is not None],**({'alive_until':upper-LONGEST_LIFE-1,'longest_life':LONGEST_LIFE,'calendar_allowance':1} if prop!='P570' else {'longest_life':LONGEST_LIFE,'calendar_allowance':1} if circa and not latest else {})})
    exclusive=max(uppers)+1 if uppers else None
    if exclusive is None or exclusive>BOUND_BEFORE:return None
    return {'year':exclusive,'semantics':'exclusive','from':'wikidata','claims':used}

def original_bound_problems(path,person,second=False,offline=False):
    proof=person.get('bound_reference')
    if not proof:
        bound=person.get('death_bound') or {}
        if person.get('qid') and person.get('died_before') is not None and isinstance(bound,dict) and bound.get('from')=='wikidata':
            entity=person.get('wikidata_evidence') or {};qid=person.get('qid');problems=[]
            if person.get('died') is not None or type(person.get('died_before')) is not int or person['died_before']>BOUND_BEFORE or bound.get('semantics')!='exclusive' or bound.get('year')!=person.get('died_before') or not re.fullmatch(r'Q[1-9]\d*',str(qid or '')) or entity.get('url')!=f'https://www.wikidata.org/wiki/Special:EntityData/{qid}.json' or not re.fullmatch(r'[a-f0-9]{64}',str(entity.get('sha256',''))) or entity.get('file')!=f"person-entity-{entity.get('sha256')}.json":return ['Wikidata bound needs exclusive-year semantics and its exact fetched entity; renew the review']
            if not offline:
                try:
                    raw=Path(cache(path,entity['file'])).read_bytes();source=json.loads(raw)['entities'][qid]
                    if hashlib.sha256(raw).hexdigest()!=entity['sha256'] or death_bound(source)!=bound:problems.append('Wikidata bound or its source entity changed')
                except (OSError,KeyError,ValueError,TypeError):problems.append('Wikidata bound source entity is unavailable')
            if second:
                review=part(path,'review.json') or {};reader=(review.get('reader') or {}).get('id');maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id');answer=(review.get('person_bounds') or {}).get(page_number_digest(person)) or {}
                if not reader or reader==maker or answer.get('reader')!=reader or len(str(answer.get('assessment','')).strip())<60:problems.append('Wikidata bound needs an independent assessment with kit.py person-bound-review')
            return problems
        if 'bound_reference' in person or (person.get('qid') and person.get('died_before') is not None and (not isinstance(person.get('death_bound'),dict) or person['death_bound'].get('from')!='wikidata')):return ['source-bound Wikidata person needs its exact reference']
        return []
    if not isinstance(proof,dict):return ['invalid source-bound Wikidata person evidence']
    problems=[];bound=person.get('death_bound') or {};qid=person.get('qid');url=proof.get('url')
    if not isinstance(bound,dict) or type(person.get('died_before')) is not int or person['died_before']>BOUND_BEFORE or person.get('died') is not None or not re.fullmatch(r'Q[1-9]\d*',str(qid or '')) or not trusted(url) or bound.get('from')!=url or bound.get('year')!=person.get('died_before') or len(str(bound.get('note','')).strip())<30 or not proof.get('name') or person.get('source')!=f'https://www.wikidata.org/wiki/{qid}' or proof.get('entity_url')!=f'https://www.wikidata.org/wiki/Special:EntityData/{qid}.json' or any(not re.fullmatch(r'[a-f0-9]{64}',str(proof.get(k,''))) for k in ('sha256','entity_sha256')) or proof.get('file')!=f"person-bound-{proof.get('sha256')}.html" or proof.get('entity_file')!=f"person-entity-{proof.get('entity_sha256')}.json":
        return ['source-bound Wikidata person needs its exact reference, entity, bound and assessment']
    if not offline:
        try:
            raw=Path(cache(path,proof['file'])).read_bytes();entity_raw=Path(cache(path,proof['entity_file'])).read_bytes();entity=json.loads(entity_raw)['entities'][qid]
            links=[urllib.parse.unquote(str(v.get('url'))).rstrip('/') for v in (entity.get('sitelinks') or {}).values()]
            if hashlib.sha256(raw).hexdigest()!=proof['sha256'] or hashlib.sha256(entity_raw).hexdigest()!=proof['entity_sha256'] or proof['name'].casefold() not in plain(raw,proof['file']).casefold() or urllib.parse.unquote(url).rstrip('/') not in links or entity.get('claims',{}).get('P570'):problems.append('source-bound Wikidata person reference or entity changed or does not identify that source')
        except (OSError,KeyError,ValueError,TypeError):problems.append('source-bound Wikidata person reference or entity is unavailable')
    if second:
        review=part(path,'review.json') or {};reader=(review.get('reader') or {}).get('id');maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id')
        answer=(review.get('person_bounds') or {}).get(page_number_digest(person)) or {}
        if not reader or reader==maker or answer.get('reader')!=reader or len(str(answer.get('assessment','')).strip())<60:problems.append('source-bound Wikidata person needs an independent assessment with kit.py person-bound-review')
    return problems

def cmd_person_bound_review(args):
    path=packet(args.packet);review=part(path,'review.json') or {};reader=(review.get('reader') or {}).get('id');maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id')
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its person-bound review')
    people=(part(path,'evidence.json') or {}).get('persons') or [];matches=[p for p in people if p.get('role')==args.role and p.get('qid')==args.qid and p.get('died_before') is not None]
    if len(matches)!=1 or not reader or reader!=args.reader or reader==maker or len(args.assessment.strip())<60:die('the independent packet reader must assess exactly one source-bound person in at least 60 characters')
    issues=original_bound_problems(path,matches[0])
    if issues:die('; '.join(issues))
    review.setdefault('person_bounds',{})[page_number_digest(matches[0])]={'reader':reader,'assessment':args.assessment};save(os.path.join(path,'review.json'),review)
    print('independent person-bound assessment recorded')

def cmd_person(args):
    """A hand in the text, by its Wikidata item: the kit reads the name and the year of death there, so that no date is typed."""
    path=packet(args.packet);ev=evidence(path)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its participants; use a separately reviewed correction or reopening')
    if args.remove:
        kept=[other for other in ev['persons'] if not(other.get('role')==args.role and (other.get('qid')==args.qid if args.qid else (other.get('name') or {}).get('en')==args.name if isinstance(other.get('name'),dict) else other.get('name')==args.name))]
        if len(kept)==len(ev['persons']):die('no such person is on record: '+'; '.join(f"{other.get('role')} {other.get('qid') or ''} {other.get('name')}" for other in ev['persons']))
        ev['persons']=kept;save(os.path.join(path,'evidence.json'),ev);write_rights(path,ev);print(f'{len(kept)} persons stay on record');return
    if args.qid:
        if not re.fullmatch(r'Q\d+',args.qid):die('a Wikidata item reads Q12345')
        entity_url=f'https://www.wikidata.org/wiki/Special:EntityData/{args.qid}.json';entity_raw=get(entity_url);entity=json.loads(entity_raw)['entities'][args.qid]
        label=lambda lang:(entity.get('labels',{}).get(lang) or {}).get('value')
        year=lambda prop:entity_year(entity,prop)
        died=year('P570');print(f"{args.qid}: {label('en')} / {label('ru')} — {(entity.get('descriptions',{}).get('en') or {}).get('value')}; born {year('P569')}, died {died}")
        person={'qid':args.qid,'name':{'en':label('en') or label('mul'),'ru':label('ru') or label('en') or label('mul')},'role':args.role,'born':year('P569'),'died':died,'source':f'https://www.wikidata.org/wiki/{args.qid}'}
        if died is None and getattr(args,'bounded',False):
            bound=death_bound(entity)
            if bound is None:die(f'{args.qid}: no claim of death, floruit or working period bounds the death before {BOUND_BEFORE}; record the person without --bounded, or with --died-before and a trusted reference')
            digest=hashlib.sha256(entity_raw).hexdigest();filename=f'person-entity-{digest}.json';Path(cache(path,filename)).write_bytes(entity_raw)
            person.update({'died_before':bound['year'],'death_bound':bound,'wikidata_evidence':{'file':filename,'url':entity_url,'sha256':digest}});print(f"died before {bound['year']} by the Wikidata claims")
        supplied=getattr(args,'died_before',None)
        if supplied is not None:
            note=getattr(args,'bound_note',None)
            if died is not None or entity.get('claims',{}).get('P570') or getattr(args,'bounded',False) or args.died is not None or supplied>BOUND_BEFORE or not args.name or not trusted(args.source) or len(str(note or '').strip())<30:die('a source-bound QID needs an unknown death, --name, trusted --source, --died-before at most 1800 and a detailed --bound-note')
            normalize_url=lambda u:urllib.parse.unquote(str(u)).rstrip('/')
            links=[normalize_url(v.get('url')) for v in (entity.get('sitelinks') or {}).values()]
            if normalize_url(args.source) not in links:die('the source must be a sitelink of this exact Wikidata entity')
            raw=get(args.source)
            if args.name.casefold() not in plain(raw,'person.html').casefold():die('the source does not contain the stated name')
            source_name='person-bound-'+hashlib.sha256(raw).hexdigest()+'.html';entity_name='person-entity-'+hashlib.sha256(entity_raw).hexdigest()+'.json'
            Path(cache(path,source_name)).write_bytes(raw);Path(cache(path,entity_name)).write_bytes(entity_raw)
            person.update({'died_before':supplied,'death_bound':{'year':supplied,'from':args.source,'note':note},'bound_reference':{'name':args.name,'url':args.source,'file':source_name,'sha256':hashlib.sha256(raw).hexdigest(),'entity_url':entity_url,'entity_file':entity_name,'entity_sha256':hashlib.sha256(entity_raw).hexdigest()}})
    elif getattr(args,'died_before',None) is not None:
        # A hand without an item or without a precise year: the upper bound of the death is the reference's, never typed from memory.
        if not(args.name and trusted(args.source)):die('--died-before needs --name and --source on a trusted site: '+', '.join(TRUSTED))
        if args.died is not None:die('--died and --died-before exclude each other')
        if args.died_before>BOUND_BEFORE:die(f'an upper bound stands in for a death only for a hand who died before {BOUND_BEFORE}')
        if len(str(getattr(args,'bound_note',None) or ''))<30:die('--bound-note: what the reference says that bounds the death (at least 30 characters), for the second reader to check')
        raw=get(args.source);content=plain(raw,'person.html').casefold()
        if args.name.casefold() not in content:die('the reference does not contain the stated name')
        filename='person-'+hashlib.sha256(raw).hexdigest()+'.html';Path(cache(path,filename)).write_bytes(raw)
        person={'name':{'en':args.name,'ru':args.ru or args.name},'role':args.role,'died':None,'died_before':args.died_before,'source':args.source,
                'death_bound':{'year':args.died_before,'from':args.source,'note':getattr(args,'bound_note',None)},'identity_evidence':{'file':filename,'url':args.source,'sha256':hashlib.sha256(raw).hexdigest()}}
        if getattr(args,'anonymous',False):person['anonymous']=True
    else:
        if not(args.name and lawful_url(args.source)):die('without an item: --name NAME --source HTTPS-URL, and --died YEAR only when supported by a trusted reference')
        if args.died is not None and not trusted(args.source):die('a stated death year requires a trusted reference: '+', '.join(TRUSTED))
        person={'name':{'en':args.name,'ru':args.ru or args.name},'role':args.role,'died':args.died,'source':args.source}
        if args.died is None:
            raw=get(args.source)
            if args.name.casefold() not in plain(raw,'person.html').casefold():die('the source does not contain the stated name')
            filename='person-'+hashlib.sha256(raw).hexdigest()+'.html'
            Path(cache(path,filename)).write_bytes(raw)
            person['identity_evidence']={'file':filename,'url':args.source,'sha256':hashlib.sha256(raw).hexdigest()}
    if args.wartime is not None:
        if not trusted(args.wartime_source):die('--wartime requires --wartime-source on a trusted site')
        person['wartime']=args.wartime=='yes';person['wartime_source']=args.wartime_source
    if args.rehabilitated is not None:
        if not trusted(args.rehabilitation_source):die('--rehabilitated requires --rehabilitation-source on a trusted site')
        person['rehabilitated']=args.rehabilitated;person['rehabilitation_source']=args.rehabilitation_source
    ev['persons']=[other for other in ev['persons'] if isinstance(other.get('name'),dict) and not(other.get('role')==args.role and other['name']==person['name'])]+[person]   # a record of an older shape is entered again, not kept
    save(os.path.join(path,'evidence.json'),ev);write_rights(path,ev);print(f"{person['name']['en']} ({args.role}) died {person['died'] if person.get('died') is not None or person.get('died_before') is None else 'before '+str(person['died_before'])}")

ROLES={'author':('автор','author'),'editor':('редактор текста','editor of the text'),'translator':('переводчик','translator')}
def legacy_rights_lines(ev):
    """The three rights sentences of a first-wave witness, written from the dates alone."""
    people=sorted(ev.get('persons') or [],key=lambda person:list(ROLES).index(person['role']));when=lambda year,ru:(f'{year}' if year>0 else f'{-year} до н. э.' if ru else f'{-year} BCE')
    return {'russia':'Общественное достояние: '+'; '.join(f"{person['name']['ru']} ({ROLES[person['role']][0]}) — год смерти {when(person['died'],True)}" for person in people)+' — ГК РФ ст. 1281 (70 лет p.m.a.) с ФЗ № 231-ФЗ от 18.12.2006, ст. 6: 50-летний срок истёк до 1 января 1993 г. (смерть до 1943 г.), произведение в общественном достоянии.',
            'usa':f"Public domain: this printing was published in {ev.get('published')} (before 1930).",
            'eu':'Public domain: '+'; '.join(f"{person['name']['en']} ({ROLES[person['role']][1]}) died {when(person['died'],False)}" for person in people)+', more than 70 years ago (Directive 2006/116/EC art. 1).'}
def rights_lines(ev):
    dates=public_domain_by_dates(ev.get('persons') or [],ev.get('published'))
    laws={'russia':'ГК РФ ст. 1281; 231-ФЗ ст. 6: срок рассчитан по датам всех участников с учётом возможного продления на четыре года.',
          'usa':'17 U.S.C. § 304: срок от публикации истёк.',
          'eu':'Directive 2006/116/EC art. 1: срок после смерти всех участников истёк.'}
    return {country:{'basis':'public-domain','law':laws[country]} for country in JURISDICTIONS if dates[country]}

def write_rights(path,ev):
    places=part(path,'places.json');entry=(places or {}).get('entry') or {};witness=(entry.get('witnesses') or {}).get(entry.get('original'))
    if witness is None:return
    old=witness.get('rights') or {};calculated=rights_lines(ev)
    rights={'reasoning':old.get('reasoning','')}
    for country in JURISDICTIONS:
        prior=old.get(country)
        if isinstance(prior,dict) and prior.get('basis') not in ('public-domain',None):rights[country]=prior
        elif country in calculated:rights[country]=calculated[country]
    witness['rights']=rights;save(os.path.join(path,'places.json'),places)

def cmd_rights(args):
    path=packet(args.packet);ev=evidence(path);write_rights(path,ev)
    places=part(path,'places.json');entry=(places or {}).get('entry') or {};witness=(entry.get('witnesses') or {}).get(entry.get('original'))
    if not witness:die('places.json has no original witness')
    if args.jurisdiction:
        if not args.basis or not args.law:die('--jurisdiction requires --basis and --law')
        witness['rights'][args.jurisdiction]={'basis':args.basis,'law':args.law}
        save(os.path.join(path,'places.json'),places)
    print(json.dumps(witness['rights'],ensure_ascii=False,indent=1))

def cmd_stamp(args):
    """The line's own record of which model read: a reader does not name itself."""
    path=packet(args.packet);name='reading.json' if args.role=='maker' else 'review.json';value=part(path,name) or die(f'no {name}')
    value['reader'].update({'model':args.model,'effort':args.effort});save(os.path.join(path,name),value);print(f"{name}: {value['reader']['id']} is {args.model} {args.effort}")

def cmd_printed(args):
    path=packet(args.packet);ev=evidence(path);ev['published']=args.year;save(os.path.join(path,'evidence.json'),ev);write_rights(path,ev);print(f'the printing used is of {args.year}')

PROOFREAD=('wikisource.org','gutenberg.org','gutenberg.ca','projekt-gutenberg.org','deutschestextarchiv.de','textgridrep.de','zeno.org','bnf.fr','archive.org','korpora.org','tufts.edu','corpusthomisticum.org','thelatinlibrary.com','libertyfund.org','hathitrust.org','umich.edu','ox.ac.uk','textcreationpartnership.org','uchicago.edu','liberliber.it','bibliotecaitaliana.it')
def proofread(url):
    host=urllib.parse.urlparse(str(url)).netloc.lower();return any(host==name or host.endswith('.'+name) for name in PROOFREAD)

def cmd_why(args):
    """Why the words are cut from a transcription that no proofreading project keeps."""
    path=packet(args.packet);ev=evidence(path);source=next((source for source in ev['sources'] if args.file in(source['file'],source.get('text'))),None) or die(f'no fetched file {args.file}')
    source['why']=args.reason;save(os.path.join(path,'evidence.json'),ev);print(f"{source['file']}: {args.reason}")

def cmd_offset(args):
    path=packet(args.packet);ev=evidence(path);ev['scans']=scans(ev);item=args.scan or (ev.get('scan') or {}).get('item')
    if item not in ev['scans']:die('name the scan first (kit.py ia)')
    ev['scans'][item]['image_offset']=args.k
    if (ev.get('scan') or {}).get('item')==item:ev['scan']=ev['scans'][item]
    save(os.path.join(path,'evidence.json'),ev);print(f'{item}: image n = OCR leaf {args.k:+d}')

def compare(path,ev,places,original):
    """Every place of the original beside the OCR; a judgment already given stays with the difference it was given to."""
    out={};every=scans(ev);read={};active=(ev.get('scan') or {}).get('item')
    def reading(item):
        if item not in read:read[item]=ocr(path,ev,every[item])
        return read[item]
    for loc,place in places['places'].items():
        text=place.get('texts',{}).get(original);old=ev['places'].get(loc,{});entry={key_:old[key_] for key_ in('transcription','dropped','from_scan','scan','whole','images','quotation','page_number_corrections') if key_ in old}
        if not text:out[loc]=entry;continue
        fixed=scan_of(ev,old).get('item') if old.get('scan') or old.get('from_scan') else None      # a place stays with its scan; a new one is looked for in each
        got=None
        for item in [fixed] if fixed in every else [active]+[item for item in every if item!=active]:
            script='zh' if original.startswith('zh') else original if original in ('la','sa') else None
            o=reading(item) if item in every else None;found=against_ocr(text,o,modern_ru=original=='ru',script=script) if o else None
            if found and (got is None or 'leaves' in found or (found.get('share') or 0)>(got.get('share') or 0)):got=found;entry['scan']=item
            if found and 'leaves' in found:break
        if not got or 'leaves' not in got:
            entry.update({'method':'image','share':(got or {}).get('share')});out[loc]=entry;continue
        printed=list(got['printed'])
        for correction in entry.get('page_number_corrections',[]):
            if correction.get('leaf') in got['leaves']:
                index=got['leaves'].index(correction['leaf'])
                if str(printed[index]) in (correction.get('from'),correction.get('to')):printed[index]=correction['to']
        if entry.get('from_scan'):               # the OCR beside itself proves nothing: the page is read instead
            entry.update({'method':'image','leaves':got['leaves'],'printed':printed});out[loc]=entry;continue
        judged={(d['transcription'],d['ocr'],d['leaf']):d for d in old.get('differences',[])}
        for d in got['differences']:
            was=judged.get((d['transcription'],d['ocr'],d['leaf']))
            if was:d.update({key_:was[key_] for key_ in('stands','note') if key_ in was})
        entry.update({'method':'ocr','share':got['share'],'leaves':got['leaves'],'printed':printed,'differences':got['differences']});out[loc]=entry
    return out

def cmd_compare(args):
    path=packet(args.packet);ev=evidence(path);places=part(path,'places.json');original=original_of(places)
    ev['places']=compare(path,ev,places,original);unfixed=set()
    for loc,entry in ev['places'].items():
        scan=scan_of(ev,entry);offset=scan.get('image_offset')
        if scan.get('ocr') and offset is None:unfixed.add(scan['item'])
        if entry.get('leaves') and offset is not None:places['places'][loc].setdefault('links',{})[original]=f"https://archive.org/details/{scan['item']}/page/n{entry['leaves'][0]+offset}/mode/1up"
        if entry.get('method')=='ocr':
            print(f"{loc}: {scan.get('item') if len(scans(ev))>1 else ''} leaves {entry['leaves']} printed {entry['printed']} {round(entry['share']*100)}% the same; "+('no difference' if not entry['differences'] else f"{len(entry['differences'])} differences"))
            for n,d in enumerate(entry['differences'],1):print(f"   {n}. leaf {d['leaf']}: transcription «{d['transcription']}» | OCR «{d['ocr']}»"+(f"  → {d['stands']} stands" if d.get('stands') else ''))
        elif entry.get('from_scan') and entry.get('leaves'):print(f"{loc}: cut from the scan’s own OCR, leaves {entry['leaves']} printed {entry['printed']}: read it whole on the page image of every leaf (image N --loc, correct what the OCR misread, then judge … whole)")
        else:print(f"{loc}: the OCR does not hold this place"+(f" ({round((entry.get('share') or 0)*100)}% found)" if entry.get('share') is not None else '')+': read it whole on the page image (image … --loc, then judge … whole)')
    save(os.path.join(path,'evidence.json'),ev);save(os.path.join(path,'places.json'),places)
    for item in sorted(unfixed):print(f'{item}: the image offset is not fixed yet: look at one image of this scan, then kit.py offset K'+(f' --scan {item}' if len(scans(ev))>1 else ''))

def original_of(places):
    return (places.get('entry') or {}).get('original') or world()['fragments']['works'].get(places['work'],{}).get('original') or die('places.json names no original language')

def cmd_judge(args):
    path=packet(args.packet);ev=evidence(path);entry=ev['places'].get(args.loc) or die(f'no compared place {args.loc} (kit.py compare)')
    if args.which=='whole':entry['whole']={'stands':args.stands,**({'note':args.note} if args.note else {})}
    else:
        chosen=[d for d in entry.get('differences',[]) if not d.get('stands')] if args.which=='all' else [entry['differences'][int(args.which)-1]]
        for d in chosen:d['stands']=args.stands;d.update({'note':args.note} if args.note else {})
    save(os.path.join(path,'evidence.json'),ev);print(f'{args.loc}: {args.which} — {args.stands} stands')

def cmd_correct(args):
    path=packet(args.packet);places=part(path,'places.json');original=original_of(places);place=places['places'].get(args.loc) or die(f'no place {args.loc}')
    text=place['texts'][original]
    if getattr(args,'from')==args.to:die('this changes nothing')
    if any(scan.get('long_s') for scan in scans(evidence(path)).values()) and getattr(args,'from').replace('ß','ss')==args.to.replace('ß','ss'):die('in a print with the long s, ſs is the print’s ß and stays ß (ſſ is ss): no correction')
    if text.count(getattr(args,'from'))!=1:die(f'«{getattr(args,"from")}» stands {text.count(getattr(args,"from"))} times in the place; give a run that stands once')
    place['texts'][original]=text.replace(getattr(args,'from'),args.to)
    places['corrections'].append({'lang':original,'from':getattr(args,'from'),'to':args.to,'evidence':args.evidence,'place':f"{places['work']} {args.loc}"})
    save(os.path.join(path,'places.json'),places);print(f'{args.loc}: corrected; run kit.py compare again')

def cmd_accepted_text_correct(args):
    """Record a new, independently reviewed correction without replacing the original acceptance."""
    path=packet(args.packet);accepted=part(path,'accepted.json') or die('an accepted packet is required')
    if not isinstance(args.to,str) or not args.to:die('the replacement cannot be empty; include surrounding context to record a deletion')
    ev=evidence(path);entry=(ev.get('places') or {}).get(args.loc) or die('no evidence for this place')
    scan=scan_of(ev,entry)
    if args.n not in (entry.get('images') or []) or args.url!=f"https://archive.org/download/{scan.get('item')}/page/n{args.n}.jpg":die('the URL must be the recorded scan page for this place')
    if not args.by or not args.second or args.by==args.second or not args.second_model or len(args.assessment.strip())<30:die('record distinct readers and the second reader’s page assessment')
    raw=Path(args.file).read_bytes() if args.file else get(args.url)
    if not valid_page_image(raw):die('the page source did not yield a readable image')
    digest=hashlib.sha256(raw).hexdigest();filename=f'text-correction-{digest}.img';Path(cache(path,filename)).write_bytes(raw)
    places=part(path,'places.json');original=original_of(places);text=places['places'][args.loc]['texts'][original]
    old=getattr(args,'from')
    if not old or text.count(old)!=1:die('the original run must occur exactly once')
    offset=text.index(old)
    cmd_correct(argparse.Namespace(packet=args.packet,loc=args.loc,to=args.to,evidence=args.assessment,**{'from':getattr(args,'from')}))
    places=part(path,'places.json');correction=places['corrections'][-1]
    correction['offset']=offset
    correction['review']={'url':args.url,'image_sha256':digest,'file':filename,'by':args.by,'second':args.second,'second_model':args.second_model,'assessment':args.assessment,'date':today()}
    accepted.setdefault('text_correction_reviews',[]).append({'loc':args.loc,'sha256':page_number_digest(correction)})
    save(os.path.join(path,'places.json'),places);save(os.path.join(path,'accepted.json'),accepted)
    print('original acceptance retained; the correction and independent assessment are bound to it')

def reverse_reviewed_correction(text,correction):
    offset=correction.get('offset');new=correction.get('to');old=correction.get('from')
    if type(offset) is not int or offset<0 or not isinstance(new,str) or not new or not isinstance(old,str) or text[offset:offset+len(new)]!=new:return None
    return text[:offset]+old+text[offset+len(new):]

def cmd_accepted_text_index(args):
    """Bind the unique position of a previously reviewed correction to its receipt."""
    path=packet(args.packet);places=part(path,'places.json');accepted=part(path,'accepted.json') or die('an accepted packet is required')
    original=original_of(places);place=(places.get('places') or {}).get(args.loc) or die('no such place');text=place['texts'][original]
    selected=[c for c in places.get('corrections') or [] if c.get('place')==f"{places['work']} {args.loc}" and c.get('review') and c.get('to')==args.to]
    if len(selected)!=1 or 'offset' in selected[0] or text.count(args.to)!=1:die('the reviewed target must be unique and not yet indexed')
    if any(c.get('review') and c.get('place')==selected[0]['place'] for c in places['corrections'][places['corrections'].index(selected[0])+1:]):die('the correction must be the latest reviewed change to this place')
    correction=selected[0];previous=page_number_digest(correction)
    receipts=[r for r in accepted.get('text_correction_reviews') or [] if r.get('loc')==args.loc and r.get('sha256')==previous]
    if len(receipts)!=1:die('the prior accepted review must match this correction')
    correction['offset']=text.index(args.to);receipts[0]['previous_sha256']=previous;receipts[0]['sha256']=page_number_digest(correction)
    save(os.path.join(path,'places.json'),places);save(os.path.join(path,'accepted.json'),accepted)
    print('unique correction position recorded; prior review digest retained')

# ---- the gate ------------------------------------------------------------------------------------------------------

NEGATES={'ru':r'\b(не|ни|нет|без|безо|никогда|никак|ничто|ничего|никто|никакой|никакая|никакое|никакие|нельзя|невозможно)\b',
         'en':r"\b(not|no|never|without|neither|nor|none|nothing|cannot|nobody|nowhere)\b|n['’]t\b|\b\w{3,}less\b",
         'de':r'\b(nicht|nichts|kein\w*|ohne|nie|niemals|nimmer|weder|niemand|nirgends)\b|\b\w{3,}los(e[nrsm]?)?\b','fr':r"\b(ne|n|pas|point|sans|ni|jamais|rien|aucune?|nul\w*|non)\b",
         'it':r'\b(non|senza|né|mai|nulla|niente|nessun\w*)\b','la':r'\b(non|ne|nec|neque|sine|nihil|haud|null\w*|nemo|numquam|nunquam|minime)\b','es':r'\b(no|sin|ni|nunca|nada|ningun\w*|jamás)\b'}
CONTRAST={'ru':r'\bне\s+(только|просто|столько|лишь)\b|\bа\s+не\b|\bвместо\b|\bнежели\b','en':r'\bnot\s+(only|merely|simply|just)\b|\brather\s+than\b|\binstead\s+of\b'}
HEDGES={'de':r'\b(vielleicht|gleichsam|gewissermaßen|eine Art|scheint|scheinen)\b','fr':r'\b(peut-être|en quelque sorte|une (sorte|espèce) de|semblent?)\b','it':r'\b(forse|una (sorta|specie) di|sembra|quasi)\b','la':r'\b(quasi|fortasse|forsitan|videtur|quodammodo)\b',
        'ru':r'\b(возможно|вероятно|по-видимому|видимо|пожалуй|как бы|в (некотором|каком-то|известном) смысле|можно сказать|так сказать|(следует|важно|стоит|нужно|необходимо) (отметить|подчеркнуть)|на самом деле|своего рода)\b',
        'en':r'\b(perhaps|arguably|maybe|seems? to|appears? to|in a sense|so to speak|it is worth noting|it is important to note|kind of|sort of)\b'}

GROUNDS={'rights':'no lawful basis established after the required source and rights searches','sources':'no scan of the work, or no text to cut the words from','reading':'the work is not the one the registers read','kit':'the kit itself fails'}

def style(at,texts,place_text,original,problems,named=None):
    """A thesis affirms: it hedges, negates or contrasts only where the words of the place do."""
    place_negates=(bool(re.search(NEGATES[original],place_text,re.I)) if original in NEGATES else True) or bool(named and named in place_text)
    place_hedges=bool(re.search(HEDGES[original],place_text,re.I)) if original in HEDGES else True
    for lang in('ru','en'):
        text=(texts or {}).get(lang) or ''
        for m in re.finditer(HEDGES[lang],text,re.I):
            if not place_hedges:problems.append(f'{at} {lang}: «{m.group(0)}» hedges while the words of the place do not; say what the place says')
        hit=re.search(CONTRAST[lang],text,re.I) or re.search(NEGATES[lang],text,re.I)
        if hit and not place_negates:problems.append(f'{at} {lang}: «{hit.group(0)}» negates while the words of the place negate nothing; say what the place affirms, or name under negation the words of the place that negate')

LINE_FAULT="line: the checkout cannot run the planting — {error}. The fault is the line's own and the packet stands as it is: write no blocked.json, change nothing, say this line in your last message and stop"

def thrown(stderr):
    """The error of a planting that died instead of answering: node prints the error and its stack, plant.mjs prints findings."""
    lines=stderr.splitlines()
    if not any(re.match(r'\s+at .+|Node\.js v',line) for line in lines):return None
    return next((line.strip() for line in lines if re.match(r'\s*\w*Error\b',line)),'node stopped with an error')

def ready():
    """Whether this checkout can plant at all: the accepted packets, tried dry. None when it can, else the reason."""
    run=subprocess.run(['node',os.path.join(REVIEW,'plant.mjs'),'--dry'],capture_output=True,text=True)
    if not run.returncode:return None
    return thrown(run.stderr) or ' / '.join((run.stdout+run.stderr).strip().splitlines()[-3:])

OWES=[]      # what an accepted packet owes to a rule younger than its acceptance: named, and settled at its next retake

# Search evidence stays with a locator, never treated as a licence to quote.
LOCATOR_SEARCHES={'internet-archive','hathitrust','google-books','wikisource','gutenberg','gutenberg-ca','gutenberg-au','gutenberg-de','zeno','neb','rgb','prl','gallica','bdh','cervantes','mdz','ddb','polona','delpher','ctext','gretil','doab','oapen','openalex','unpaywall','crossref','philarchive','universities','author','publisher','institutes'}

ADDRESS_KINDS=('section','chapter','paragraph','folio','leaf','verse','manuscript')
def valid_address(address):
    return isinstance(address,dict) and address.get('kind') in ADDRESS_KINDS and isinstance(address.get('value'),str) and bool(address['value'].strip()) and lawful_url(address.get('url'))

def cmd_address(args):
    path=packet(args.packet);places=part(path,'places.json');place=(places.get('places') or {}).get(args.loc)
    if not place:die('no such place in this packet')
    address={'kind':args.kind,'value':args.value,'url':args.url}
    if not valid_address(address):die('address needs a kind, a value and a lawful source URL')
    original=original_of(places);place['address']=address;place.setdefault('links',{})[original]=args.url
    save(os.path.join(path,'places.json'),places);print(f'{args.loc}: structured source address recorded')

def cmd_locator(args):
    path=packet(args.packet);places=part(path,'places.json');ev=evidence(path)
    if not lawful_url(args.url):die('the book locator must be a lawful https URL')
    searches=load(args.searches)
    if not isinstance(searches,list):die('searches must be a JSON list')
    place=places['places'].setdefault(args.loc,{'label':{'ru':args.ru,'en':args.en},'texts':{},'links':{},'refs':[]})
    place['url']=args.url;place['opens']='book';place['label']={'ru':args.ru,'en':args.en}
    original=original_of(places);place['refs']=[[original,args.reference]]
    ev['places'].setdefault(args.loc,{})['locator']={'url':args.url,'reference':args.reference,'searches':searches,'maker_read':args.maker_read}
    save(os.path.join(path,'places.json'),places);save(os.path.join(path,'evidence.json'),ev)
    print(f'{args.loc}: locator and searches recorded')

def missing_search_sites(searches):
    valid={item.get('site') for item in searches if isinstance(item,dict) and lawful_url(item.get('url','')) and len(str(item.get('outcome','')))>=15}
    return LOCATOR_SEARCHES-valid

def missing_language_problems(record):
    if not isinstance(record,dict):return ['no search record']
    problems=[];missing=missing_search_sites(record.get('tried') or [])
    if missing:problems.append('search outcomes missing for '+', '.join(sorted(missing)))
    if len(str(record.get('reason','')).strip())<30:problems.append('a reason for the missing text is required')
    if len(str(record.get('edition','')).strip())<15 or not lawful_url(record.get('book_url')):problems.append('edition and lawful book locator are required')
    return problems

def locator_problems(loc,place,ev):
    problems=[];proof=(ev.get('places',{}).get(loc) or {}).get('locator') or {}
    if not lawful_url(proof.get('url','')) or proof.get('url')!=place.get('url'):problems.append(f'{loc}: book locator URL is missing')
    if len(str(proof.get('reference','')))<8:problems.append(f'{loc}: edition and page reference are missing')
    if len(str(proof.get('maker_read','')))<30:problems.append(f'{loc}: maker must explain how the book was read')
    searches=proof.get('searches') or []
    missing=missing_search_sites(searches)
    if missing:problems.append(f'{loc}: search outcomes missing for '+', '.join(sorted(missing)))
    return problems

def valid_page_image(raw):
    try:
        from PIL import Image
        with Image.open(io.BytesIO(raw)) as image:
            if image.format not in ('JPEG','PNG','WEBP'):return False
            image.verify()
        return True
    except ImportError:
        try:return subprocess.run(['identify','-'],input=raw,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=20).returncode==0
        except (OSError,subprocess.TimeoutExpired):
            return (raw.startswith(b'\x89PNG\r\n\x1a\n') and raw.endswith(b'IEND\xaeB`\x82')) or (raw.startswith(b'\xff\xd8\xff') and raw.endswith(b'\xff\xd9')) or (len(raw)>20 and raw.startswith(b'RIFF') and raw[8:12]==b'WEBP')
    except Exception:return False

def page_number_digest(correction):
    return hashlib.sha256(json.dumps(correction,sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()

def page_number_problems(path,ev,loc,entry,offline=False):
    problems=[];accepted=part(path,'accepted.json') or {};reviews=accepted.get('page_number_reviews') or []
    for correction in entry.get('page_number_corrections') or []:
        leaf=correction.get('leaf');leaves=entry.get('leaves') or [];printed=entry.get('printed') or []
        scan=scan_of(ev,entry);offset=scan.get('image_offset');item=scan.get('item')
        if leaf not in leaves or leaves.index(leaf)>=len(printed) or printed[leaves.index(leaf)]!=correction.get('to'):problems.append(f'{loc}: corrected page number does not match the leaf');continue
        expected=f'https://archive.org/download/{item}/page/n{leaf+offset}.jpg' if type(leaf) is int and type(offset) is int and item else ''
        if correction.get('url')!=expected or not re.fullmatch(r'[a-f0-9]{64}',str(correction.get('image_sha256',''))) or correction.get('file')!=f"page-number-{correction.get('image_sha256')}.img" or not correction.get('from') or not correction.get('to') or correction.get('from')==correction.get('to'):problems.append(f'{loc}: page correction needs the exact scan image and both page numbers')
        if not correction.get('by') or not correction.get('second') or correction.get('by')==correction.get('second') or not correction.get('second_model') or len(str(correction.get('assessment','')).strip())<30:problems.append(f'{loc}: page correction needs a distinct second reader and assessment')
        if not any(review.get('loc')==loc and review.get('sha256')==page_number_digest(correction) for review in reviews):problems.append(f'{loc}: page correction is not bound to the accepted packet review')
        if not offline:
            try:
                raw=Path(cache(path,correction['file'])).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=correction.get('image_sha256') or not valid_page_image(raw):problems.append(f'{loc}: page correction image changed or is invalid')
            except (OSError,KeyError):problems.append(f'{loc}: page correction image is unavailable')
    return problems

def cmd_page_number_correct(args):
    """Correct a scan's page-number metadata after two readers inspect the image."""
    path=packet(args.packet);ev=evidence(path);accepted=part(path,'accepted.json')
    if not accepted:die('page-number correction requires an accepted packet')
    if not args.by or not args.second or args.by==args.second or not args.second_model or len(args.assessment.strip())<30:die('record distinct readers and the second reader’s page assessment')
    if not args.loc or len(set(args.loc))!=len(args.loc):die('choose distinct places')
    entries=[]
    for loc in args.loc:
        entry=(ev.get('places') or {}).get(loc) or die(f'{loc}: no evidence')
        scan=scan_of(ev,entry);leaves=entry.get('leaves') or [];printed=entry.get('printed') or [];offset=scan.get('image_offset')
        if args.leaf not in leaves or leaves.index(args.leaf)>=len(printed) or str(printed[leaves.index(args.leaf)])!=args.old:die(f'{loc}: current page number does not match the stated predecessor')
        if type(offset) is not int or args.url!=f"https://archive.org/download/{scan.get('item')}/page/n{args.leaf+offset}.jpg":die(f'{loc}: URL must identify this exact scan leaf')
        if any(c.get('leaf')==args.leaf for c in entry.get('page_number_corrections') or []):die(f'{loc}: this leaf already has a page-number correction')
        entries.append((loc,entry,leaves.index(args.leaf)))
    if not args.old or not args.new or args.old==args.new:die('give distinct old and observed page numbers')
    raw=Path(args.file).read_bytes() if args.file else get(args.url)
    if not valid_page_image(raw):die('the page source did not yield a readable image')
    digest=hashlib.sha256(raw).hexdigest();filename=f'page-number-{digest}.img';Path(cache(path,filename)).write_bytes(raw)
    for loc,entry,index in entries:
        correction={'leaf':args.leaf,'from':args.old,'to':args.new,'url':args.url,'image_sha256':digest,'file':filename,'by':args.by,'second':args.second,'second_model':args.second_model,'assessment':args.assessment,'date':today()}
        entry.setdefault('page_number_corrections',[]).append(correction);entry['printed'][index]=args.new
        accepted.setdefault('page_number_reviews',[]).append({'loc':loc,'sha256':page_number_digest(correction)})
    save(os.path.join(path,'evidence.json'),ev);save(os.path.join(path,'accepted.json'),accepted)
    print(f"{len(entries)} page-number corrections recorded; original acceptance and source predecessor retained")

def cmd_page(args):
    """Keep a page from a lawful non-Archive source without modifying its bytes."""
    path=packet(args.packet);ev=evidence(path)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its page evidence')
    if not lawful_url(args.url):die('page source must be a lawful https URL')
    if args.loc not in (part(path,'places.json') or {}).get('places',{}):die('no such place in the packet')
    raw=Path(args.file).read_bytes() if args.file else get(args.url)
    if not valid_page_image(raw):die('the page source did not yield a readable image')
    digest=hashlib.sha256(raw).hexdigest();key=hashlib.sha256(args.url.encode()).hexdigest()[:16];name=f'external-n{args.n}-{args.by}-{key}-{digest}.img';destination=Path(cache(path,name))
    if destination.exists() and destination.read_bytes()!=raw:die('page filename already holds different bytes')
    if not destination.exists():destination.write_bytes(raw)
    scan={'item':'external','ocr':False,'image_offset':0};ev['scans']=scans(ev);ev['scans']['external']=scan
    if not ev.get('scan'):ev['scan']=scan
    already=any(image.get('file')==name and image.get('by')==args.by and image.get('loc')==args.loc for image in ev.get('images',[]))
    ev['images']=[image for image in ev.get('images',[]) if not(image.get('file')==name and image.get('by')==args.by and image.get('loc')==args.loc)]
    ev['images'].append({'item':'external','n':args.n,'file':name,'url':args.url,'sha256':hashlib.sha256(raw).hexdigest(),'by':args.by,'loc':args.loc})
    if args.by=='maker':
        place=ev['places'].setdefault(args.loc,{})
        previous=dict(place);same=scan_of(ev,place).get('item')=='external'
        place.update({'method':'image','scan':'external','images':sorted(set(place.get('images',[]) if same else [])|{args.n})})
        if previous!=place or not already:
            review=part(path,'review.json')
            if previous:ev.setdefault('page_rebindings',[]).append({'loc':args.loc,'from':previous,'review':review,'date':today()})
            if review is not None:save(os.path.join(path,'review.json'),{})
    save(os.path.join(path,'evidence.json'),ev)
    print(f"{cache(path,name)}\nsha256 {hashlib.sha256(raw).hexdigest()}: read the page image")

def cmd_page_withdraw(args):
    """Retain an incorrectly registered external image and require replacement."""
    path=packet(args.packet);ev=evidence(path)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its page evidence')
    matches=[image for image in ev.get('images') or [] if image.get('loc')==args.loc and image.get('file')==args.file and image.get('item')=='external']
    if len(matches)!=1 or len(args.reason.strip())<40:die('choose one external image for this place and give a specific reason')
    image=matches[0]
    replacement_n=args.replacement_page if args.replacement_page is not None else image.get('n')
    if type(replacement_n) is not int or replacement_n<1:die('give a positive replacement page index')
    try:raw=Path(cache(path,image['file'])).read_bytes()
    except (OSError,KeyError):die('the recorded page bytes are missing')
    if hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):die('the recorded page image changed or is invalid')
    entry=(ev.get('places') or {}).get(args.loc) or {};previous=json.loads(json.dumps(entry))
    ev['images'].remove(image)
    if image.get('by','maker')=='maker' and not any(other.get('loc')==args.loc and other.get('n')==image.get('n') and other.get('by','maker')=='maker' for other in ev['images']):entry['images']=[n for n in entry.get('images') or [] if n!=image.get('n')]
    ev.setdefault('withdrawn_page_images',[]).append({'image':image,'place':previous,'replacement_n':replacement_n,'reason':args.reason,'review':part(path,'review.json') or {},'date':today()})
    save(os.path.join(path,'evidence.json'),ev)
    if part(path,'review.json') is not None:save(os.path.join(path,'review.json'),{})
    print('external page record withdrawn with its bytes and review retained; register the actual replacement and reread')

def withdrawn_page_problems(path,ev,locs,second=False,offline=False):
    problems=[]
    for retired in ev.get('withdrawn_page_images') or []:
        image=retired.get('image') or {};loc=image.get('loc')
        if loc not in locs:continue
        if image.get('item')!='external' or not image.get('file') or type(retired.get('replacement_n')) is not int or retired['replacement_n']<1 or len(str(retired.get('reason','')).strip())<40:problems.append(f'{loc}: invalid withdrawn page history');continue
        if not offline:
            try:
                raw=Path(cache(path,image['file'])).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=image.get('sha256'):problems.append(f'{loc}: withdrawn page evidence changed')
            except OSError:problems.append(f'{loc}: withdrawn page evidence is missing')
        if image.get('by','maker')=='second' and not second:continue
        if not any(other.get('loc')==loc and other.get('n')==retired['replacement_n'] and other.get('by','maker')==image.get('by','maker') and other.get('file')!=image.get('file') for other in ev.get('images') or []):problems.append(f'{loc}: withdrawn {image.get("by","maker")} page needs a replacement')
    return problems

def cmd_transcribe_original_page(args):
    path=packet(args.packet)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot be changed')
    places=part(path,'places.json') or {};reading=part(path,'reading.json') or {};ev=evidence(path)
    maker=(reading.get('reader') or {}).get('id');original=original_of(places)
    if not maker or args.reader!=maker or not original:die('the packet maker and original language must be recorded first')
    if args.loc in (places.get('places') or {}) or args.loc in (ev.get('places') or {}):die('this place or its evidence already exists; preserve it and use a reviewed correction')
    if not lawful_url(args.url) or args.n<1 or not args.label_ru.strip() or not args.label_en.strip() or len(args.why.strip())<60 or len(args.punctuation.strip())<30:die('give a lawful image, positive page, labels, search reason and punctuation account')
    text=unicodedata.normalize('NFC',args.text.replace('\r\n','\n').replace('\r','\n').strip())
    if not text:die('transcribe the original words from the page')
    raw=Path(args.file).read_bytes() if args.file else get(args.url)
    if not valid_page_image(raw):die('the source did not yield a readable page image')
    img_sha=hashlib.sha256(raw).hexdigest();text_raw=text.encode('utf-8');text_sha=hashlib.sha256(text_raw).hexdigest()
    key=hashlib.sha256((args.url+'\0'+args.loc).encode()).hexdigest()[:16]
    img_name=f'human-page-{img_sha}-maker.img';text_name=f'human-text-{key}-{img_sha}-{text_sha}.txt'
    Path(cache(path,img_name)).write_bytes(raw);Path(cache(path,text_name)).write_bytes(text_raw)
    source={'role':'transcription','kind':'human-page','url':args.url,'file':text_name,'text':text_name,'sha256':text_sha,'recorded':today(),'why':args.why,'human_page':{'loc':args.loc,'n':args.n,'image_file':img_name,'image_sha256':img_sha,'transcriber':maker,'punctuation':args.punctuation}}
    ev['sources']=[item for item in ev.get('sources',[]) if item.get('file')!=text_name]+[source]
    scan={'item':'external','ocr':False,'image_offset':0};ev['scans']=scans(ev);ev['scans']['external']=scan
    if not ev.get('scan'):ev['scan']=scan
    image={'item':'external','n':args.n,'file':img_name,'url':args.url,'sha256':img_sha,'by':'maker','loc':args.loc}
    ev['images']=[item for item in ev.get('images',[]) if not(item.get('file')==img_name and item.get('loc')==args.loc)]+[image]
    ev.setdefault('places',{})[args.loc]={'transcription':text_name,'method':'image','scan':'external','images':[args.n]}
    places.setdefault('places',{})[args.loc]={'label':{'ru':args.label_ru,'en':args.label_en},'texts':{original:text},'links':{original:args.url},'refs':[]}
    save(os.path.join(path,'evidence.json'),ev);save(os.path.join(path,'places.json'),places)
    print('human page transcription and image recorded; whole-page judgement and independent reading are required')

def human_page_scope(path,source):
    proof=source.get('human_page') or {};loc=proof.get('loc')
    return page_number_digest({'source':source,'place':((part(path,'places.json') or {}).get('places') or {}).get(loc)})

def human_page_problems(path,ev,reading,places,second=False,offline=False):
    problems=[];review=part(path,'review.json') or {};maker=(reading.get('reader') or {}).get('id')
    for source in ev.get('sources') or []:
        if source.get('kind')!='human-page':continue
        proof=source.get('human_page') or {}
        if not isinstance(proof,dict):problems.append('invalid human-page transcription provenance');continue
        loc=proof.get('loc');entry=(ev.get('places') or {}).get(loc) or {}
        users=[name for name,item in (ev.get('places') or {}).items() if item.get('transcription')==source.get('file') and name in (places.get('places') or {})]
        if users and users!=[loc]:problems.append('human-page transcription cannot be attributed to a different place');continue
        if loc not in (places.get('places') or {}):continue
        images=ev.get('images') or [];same=lambda image,by:image.get('by')==by and image.get('loc')==loc and image.get('n')==proof.get('n') and image.get('url')==source.get('url') and image.get('sha256')==proof.get('image_sha256')
        maker_image=next((image for image in images if same(image,'maker') and image.get('file')==proof.get('image_file')),None)
        if source.get('role')!='transcription' or source.get('file')!=source.get('text') or not lawful_url(source.get('url')) or proof.get('transcriber')!=maker or not maker or type(proof.get('n')) is not int or proof['n']<1 or len(str(proof.get('punctuation','')).strip())<30 or entry.get('transcription')!=source.get('file') or not maker_image:problems.append(f'{loc}: human transcription needs its named maker, exact page and punctuation account');continue
        if not offline:
            try:
                image=Path(cache(path,maker_image['file'])).read_bytes();text=Path(cache(path,source['file'])).read_bytes()
                if hashlib.sha256(image).hexdigest()!=proof.get('image_sha256') or not valid_page_image(image) or hashlib.sha256(text).hexdigest()!=source.get('sha256'):problems.append(f'{loc}: human transcription or page image changed')
            except (OSError,KeyError):problems.append(f'{loc}: human transcription or page image is missing')
        if second:
            reader=(review.get('reader') or {}).get('id');check=(review.get('human_pages') or {}).get(loc) or {}
            looked=any(item.get('loc')==loc and item.get('n')==proof.get('n') and item.get('reads') in ('as the packet','differs') for item in review.get('images') or [])
            second_image=next((image for image in images if same(image,'second')),None)
            if not second_image or not looked or not reader or reader==maker or check.get('reader')!=reader or check.get('scope_sha256')!=human_page_scope(path,source) or len(str(check.get('assessment','')).strip())<60:problems.append(f'{loc}: independent reader must inspect the same human-transcribed page and its punctuation')
            elif not offline:
                try:
                    raw=Path(cache(path,second_image['file'])).read_bytes()
                    if hashlib.sha256(raw).hexdigest()!=proof.get('image_sha256') or not valid_page_image(raw):problems.append(f'{loc}: second reader page image changed')
                except (OSError,KeyError):problems.append(f'{loc}: second reader page image is missing')
    return problems

def cmd_original_page_review(args):
    path=packet(args.packet);ev=evidence(path);review=part(path,'review.json') or {};reader=(review.get('reader') or {}).get('id');reading=part(path,'reading.json') or {};maker=(reading.get('reader') or {}).get('id')
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its review')
    found=[source for source in ev.get('sources') or [] if source.get('kind')=='human-page' and (source.get('human_page') or {}).get('loc')==args.loc]
    if len(found)!=1 or not reader or reader!=args.reader or reader==maker or len(args.assessment.strip())<60:die('the independent reader must assess exactly one human page transcription')
    review.setdefault('human_pages',{})[args.loc]={'reader':reader,'scope_sha256':human_page_scope(path,found[0]),'assessment':args.assessment}
    save(os.path.join(path,'review.json'),review);print('independent human-page assessment recorded')

def cmd_quotation(args):
    path=packet(args.packet);ev=evidence(path)
    if args.loc not in (part(path,'places.json') or {}).get('places',{}):die('the place is not in the packet')
    if not re.fullmatch(r'[a-f0-9]{64}',args.sha256):die('the page digest must be a SHA-256')
    if not lawful_url(args.source):die('the source must be a lawful https URL')
    if not any(image.get('sha256')==args.sha256 and image.get('by','maker')=='maker' for image in ev.get('images',[])):die('fetch the page image as maker first')
    ev['places'].setdefault(args.loc,{})['quotation']={'source':args.source,'page':args.page,'lines':args.lines,'sha256':args.sha256,**({'justification':args.justification} if args.justification else {})}
    save(os.path.join(path,'evidence.json'),ev)
    print(f'{args.loc}: quotation evidence recorded')

def quotation_problems(loc,text,proof,ev,second=False,path=None,offline=False):
    problems=[]
    if not isinstance(proof,dict):return [f'place {loc}: quotation needs a source, page, lines and image digest']
    for field in ('source','page','lines','sha256'):
        if not proof.get(field):problems.append(f'place {loc}: quotation {field} is missing')
    if not lawful_url(proof.get('source','')):problems.append(f'place {loc}: quotation source must be a lawful https URL')
    digest=proof.get('sha256','')
    if not re.fullmatch(r'[a-f0-9]{64}',str(digest)):problems.append(f'place {loc}: quotation image SHA-256 is invalid')
    elif not any(image.get('sha256')==digest and image.get('by','maker')=='maker' for image in ev.get('images',[])):problems.append(f'place {loc}: the maker has no matching fetched page image')
    maker=next((image for image in ev.get('images',[]) if image.get('sha256')==digest and image.get('by','maker')=='maker'),None)
    reader=((part(path,'review.json') or {}).get('reader') or {}).get('id') if path else None
    matched=second_witness_image({'images':ev.get('images') or []},{'url':maker.get('url'),'image_sha256':digest},reader) if maker else None
    if second and not matched:problems.append(f'place {loc}: the second reader must fetch or capture the same page')
    if path and not offline:
        for by in (('maker','second') if second else ('maker',)):
            image=maker if by=='maker' else matched
            if image:
                try:
                    if (raw:=Path(cache(path,image['file'])).read_bytes()) and (hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw)):problems.append(f'place {loc}: {by} image changed or is invalid')
                    elif not raw:problems.append(f'place {loc}: {by} image is empty')
                except (OSError,KeyError):problems.append(f'place {loc}: {by} image is missing locally')
    # Sentence endings are an upper-bound aid. The printed page still decides the boundaries.
    count=len(re.findall(r'(?:[.!?…]+(?:[”»’」』\"\')\]]+)?(?=\s|$)|[。！？؟।॥]+[”»’」』\"\')\]]*)',text))
    if re.search(r'[\u0370-\u03ff\u1f00-\u1fff]',text):count+=text.count(';')
    if (len(text)>900 or count>3) and len(str(proof.get('justification',''))) < 30:problems.append(f'place {loc}: quotation exceeds 900 characters or three sentences and needs a justification')
    return problems

def language_coverage(path,entry,original,legacy_work=False,held=False,second=False,offline=False):
    problems=[];debts=[]
    missing_by_lang={item.get('language'):item for item in entry.get('absent') or [] if isinstance(item,dict)}
    for lang in ('ru','en','es','de','zh-Hans'):
        if lang==original:continue
        file=witness_file(path,lang)
        search=language_search_file(path,lang)
        if held and lang in (entry.get('witnesses') or {}) and not os.path.isfile(file):
            places=(part(path,'places.json') or {}).get('places') or {}
            if places:
                existing=world()['fragments'].get('places',{}).get(work_of(path),{})
                for loc in sorted(places):
                    if not (existing.get(loc,{}).get('texts') or {}).get(lang):debts.append(f'{lang} {loc}: no accepted translation or documented search for this place')
            continue
        if not os.path.isfile(file) and os.path.isfile(search) and (not legacy_work or os.path.isfile(search+'.accepted')):
            raw=Path(search).read_bytes()
            if os.path.isfile(search+'.accepted') and load(search+'.accepted').get('sha256')!=hashlib.sha256(raw).hexdigest():problems.append(f'{lang}: search changed since acceptance');continue
            # A translation accepted later, in an extension of this work, takes precedence over the accepted search that
            # found none: the search stays as the record of its day and the work holds the witness.
            if os.path.isfile(search+'.accepted') and lang in ((((world()['fragments'].get('works') or {}).get(work_of(path)) or {}).get('witnesses')) or {}):continue
            problems.extend(f'{lang}: {issue}' for issue in language_search_problems(path,lang,json.loads(raw),second=second))
            continue
        if os.path.isfile(file):
            marker=file+'.accepted'
            if legacy_work and not os.path.isfile(marker):
                debts.append(f'{lang}: witness is research until independently accepted')
                continue
            raw=Path(file).read_bytes()
            if os.path.isfile(marker) and load(marker).get('sha256')!=hashlib.sha256(raw).hexdigest():problems.append(f'{lang}: witness changed since acceptance');continue
            data=json.loads(raw)
            problems.extend(f'{lang}: {issue}' for issue in witness_problems(path,lang,data,second=second,offline=offline))
            places=(part(path,'places.json') or {}).get('places') or {}
            for loc in sorted(set(places)-set(data.get('places') or {})):
                gaps=data.get('missing_places') or {}
                (debts if legacy_work else problems).extend(f'{lang} {loc}: {issue}' for issue in missing_language_problems(gaps.get(loc)))
        else:
            issues=missing_language_problems(missing_by_lang.get(lang))
            (debts if legacy_work else problems).extend(f'{lang}: {issue}' for issue in issues)
    return problems,debts

def gate(path,second=False,offline=False):
    problems=[];work=work_of(path);add=problems.append
    owe=OWES.append if part(path,'accepted.json') is not None else add
    blocked=part(path,'blocked.json')
    if blocked is not None:
        if len(str(blocked.get('reason','')))<40 or not blocked.get('tried'):add('blocked.json: give the reason in a full sentence and list under tried what was looked at (with URLs)')
        if blocked.get('ground') not in GROUNDS:add('blocked.json: ground wants one of '+'; '.join(f'{name} ({what})' for name,what in GROUNDS.items()))
        if blocked.get('ground')=='sources':
            scope=blocked.get('source_scope','original')
            if scope not in ('original','translation'):add('blocked.json: source_scope must be original or translation')
            if scope=='translation':
                language=blocked.get('language');original=((part(path,'places.json') or {}).get('entry') or {}).get('original')
                fragments=os.path.join(HOME,'fragments.json')
                if not original and os.path.isfile(fragments):original=((load(fragments).get('works') or {}).get(work) or {}).get('original')
                tried=blocked.get('tried') or []
                if not original or language not in ('ru','en','es','de','zh-Hans') or language==original:add('blocked.json: translation source scope needs a known original and a different interface language')
                if not isinstance(tried,list) or not tried or any(not isinstance(item,dict) or not lawful_url(item.get('url')) or len(str(item.get('outcome','')).strip())<20 for item in tried):add('blocked.json: translation source scope needs searched HTTPS sources with specific outcomes')
            elif scope=='original' and any(scan.get('ocr') for scan in scans(part(path,'evidence.json') or {}).values()):
                add('blocked.json: the packet holds a scan with page-indexed OCR, so the words can be cut from the page itself (kit.py cut . ocr …, MAKER.md step 3). A project whose site did not answer is named in kit.py why; it does not close this road. Take blocked.json away and make the packet')
        if blocked.get('ground')=='rights':
            ev=part(path,'evidence.json') or {};people=ev.get('persons') or [];printed=ev.get('published')
            if people and all(public_domain_by_dates(people,printed).values()):
                add('blocked.json: all three jurisdictions are public domain by the recorded dates; take the block away and make the packet')
        return problems,'blocked' if not problems else 'shut'
    places,reading,ev=part(path,'places.json'),part(path,'reading.json'),part(path,'evidence.json')
    for name,value in(('places.json',places),('reading.json',reading),('evidence.json',ev)):
        if value is None:add(f'{name} is missing')
    if problems:return problems,'shut'
    problems.extend(withdrawn_page_problems(path,ev,(places.get('places') or {}).keys(),second=second,offline=offline))
    for person in ev.get('persons') or []:problems.extend(original_bound_problems(path,person,second=second,offline=offline))
    w=world(but=(reading.get('reader') or {}).get('id'))
    if work not in w['works']:return [f'{work} is not a work of the layer'],'shut'
    entry=places.get('entry');full_task=task(w,work);t=scoped_task(path,full_task)
    scope=part(path,'scope.json')
    if scope:
        waiting={item['key'] for item in full_task['readings']+full_task['links']}
        if not scope.get('keys') or any(key not in waiting for key in scope['keys']):add('extension scope includes a key that is no longer waiting')
    held=None if entry else w['fragments']['works'].get(work)        # a packet with an entry brings the whole work; one without adds places to a work the bonsai holds
    if places.get('work')!=work:add('places.json names another work')
    provenance=w['fragments'].get('provenance') or {}
    extension_places=(provenance.get('extension_places') or {}).get(work,{}) if part(path,'accepted.json') and packet_selector(path)==work and work in (provenance.get('packets') or []) else {}
    if entry and set(w['fragments']['places'].get(work,{}))-set(places.get('places') or {})-set(extension_places):add('the bonsai already holds places of this work: drop entry from places.json and add places of the same edition')
    if not held and not entry:add('places.json has no entry for the work')
    entry=entry or held or {};original=entry.get('original') or ''
    original_rights=((entry.get('witnesses') or {}).get(original) or {}).get('rights') or {}
    locator_only=all(isinstance(original_rights.get(country),dict) and original_rights[country].get('basis')=='locator-only' for country in JURISDICTIONS)
    if held and (ev.get('mexico_quotation') or any(isinstance(original_rights.get(country),dict) and original_rights[country].get('basis')=='short-quotation' for country in JURISDICTIONS)):
        witness={**((entry.get('witnesses') or {}).get(original) or {})};witness.pop('mexico_quotation',None)
        if ev.get('mexico_quotation'):witness['mexico_quotation']=ev['mexico_quotation']
        issues=mexico_problems(witness,places.get('places') or {},part(path,'review.json') or {},(reading.get('reader') or {}).get('id'),second,path=path,offline=offline)
        for issue in issues:(owe if not ev.get('mexico_quotation') or (part(path,'accepted.json') and issue=='Mexico quotation assessment needs an explicit review outcome') else add)(issue)
    # the witness and the rights
    if not held:
        for name in('author','title'):
            if not all((entry.get(name) or {}).get(lang) for lang in('ru','en')):add(f'entry.{name} wants ru and en')
        if not re.fullmatch(r'[a-z]{2,3}(?:-[A-Z][a-z]{3})?',original):add('entry.original wants a language code (de, en, ru, zh-Hans …)')
        witnesses=entry.get('witnesses') or {}
        if list(witnesses)!=[original]:add('entry.witnesses holds one witness in this pass: the original, under its language code')
        witness=witnesses.get(original) or {}
        for name in('edition','licence','attribution'):
            if not isinstance(witness.get(name),str) or len(witness.get(name,''))<20:add(f'witness.{name} is missing or too short to name the book')
        if not isinstance(witness.get('year'),int):add('witness.year wants the year of the printing as a number')
        if not lawful_url(witness.get('url','')):add('witness.url wants a lawful https address of the scan')
        from_scan=bool(ev.get('places')) and all(entry.get('from_scan') for entry in ev['places'].values())
        web_urls={s.get('url') for s in ev.get('sources',[]) if s.get('file') in {e.get('transcription') for e in ev.get('places',{}).values()}}
        web_original=bool(ev.get('places')) and all(entry.get('scan')=='web' for entry in ev['places'].values()) and bool(web_urls) and web_urls<=set(witness.get('sources') or []) and not original_browser_problems(path,ev,reading,places,second=second,offline=offline)
        if not locator_only and len([s for s in witness.get('sources') or [] if lawful_url(s)])<(1 if from_scan or web_original else 2):add('witness.sources wants the scan and the transcription (two https addresses; one for scan OCR or a verified born-digital original)')
        rights=witness.get('rights') or {}
        legacy=all(isinstance(rights.get(country),str) for country in JURISDICTIONS) and rights.get('basis')=='public-domain'
        if legacy:owe('witness rights use the older single-basis form; record a basis and law for each jurisdiction')
        else:
            for country in JURISDICTIONS:
                item=rights.get(country) or {}
                if not isinstance(item,dict) or item.get('basis') not in BASES or len(str(item.get('law',''))) < 20:add(f'witness.rights.{country} wants a basis and a law statement')
        if len(str(rights.get('reasoning','')))<60:add('witness.rights.reasoning wants its sentences: whose text, which transcription (and which edition it follows), which scan, what was normalised')
        if witness.get('mexico_quotation') or any(isinstance(rights.get(country),dict) and rights[country].get('basis')=='short-quotation' for country in JURISDICTIONS):
            issues=mexico_problems(witness,places.get('places') or {},part(path,'review.json') or {},(reading.get('reader') or {}).get('id'),second,path=path,offline=offline)
            for issue in issues:(owe if not witness.get('mexico_quotation') or (part(path,'accepted.json') and issue=='Mexico quotation assessment needs an explicit review outcome') else add)(issue)
        problems.extend(license_problems(witness,ev.get('sources') or [],lambda name:cache(path,name),part(path,'review.json') or {},(reading.get('reader') or {}).get('id'),second,offline))
        for item in entry.get('absent') or []:
            if not item.get('tried'):
                owe(f"entry.absent {item.get('language')}: record searches with outcomes, or leave the language unsearched")
        persons=ev.get('persons') or []
        if not any(person.get('role')=='author' for person in persons):add('evidence.persons wants the author with the year of death and the page that gives it')
        sound=True
        for person in persons:
            identity=person.get('identity_evidence') or {}
            reference=(person.get('died') is None and lawful_url(person.get('source')) and (person.get('died_before') is None or trusted(person.get('source')) or person.get('qid')) and identity.get('url')==person.get('source') and bool(re.fullmatch(r'[a-f0-9]{64}',str(identity.get('sha256','')))))
            if reference and not offline:
                try:
                    raw=Path(cache(path,identity['file'])).read_bytes()
                    name=(person.get('name') or {}).get('en','')
                    reference=bool(name) and hashlib.sha256(raw).hexdigest()==identity['sha256'] and name.casefold() in plain(raw,identity['file']).casefold()
                except (OSError,KeyError,TypeError):reference=False
            if person.get('died_before') is not None and death_year(person) is None:sound=False;add(f"evidence.persons {person.get('name')}: an upper bound of the death needs died_before ≤ {BOUND_BEFORE}, no died, and death_bound with its source")
            if (person.get('died') is not None and type(person.get('died')) is not int) or not isinstance(person.get('name'),dict) or not(person.get('qid') or (not identity and trusted(person.get('source'))) or reference):sound=False;add(f"evidence.persons {person.get('name')}: enter the person again with kit.py person ROLE QID or a source for the unknown-dated hand")
            if person.get('wartime') is not None and (type(person['wartime']) is not bool or not trusted(person.get('wartime_source'))):sound=False;add('wartime status requires a trusted source')
            if person.get('rehabilitated') is not None and (type(person['rehabilitated']) is not int or not trusted(person.get('rehabilitation_source'))):sound=False;add('rehabilitation requires a year and a trusted source')
        if sound and persons:
            for author in layer_authors(w,work):
                if not any(person.get('role')=='author' and same_participant(author,person) for person in persons):
                    owe(f"evidence.persons: the layer author {author['name'].get('en') or author['name'].get('ru')} is missing or needs identity reconciliation")
        if not isinstance(ev.get('published'),int):add('evidence.published wants the year of the printing used')
        elif ev['published']!=witness.get('year'):add('evidence.published and witness.year differ')
        if sound and persons:
            if legacy:
                if not all(public_domain_by_dates(persons,ev.get('published')).values()) or {name:rights.get(name) for name in JURISDICTIONS}!=legacy_rights_lines(ev):add('legacy public-domain rights do not follow from the recorded dates')
            else:
                dates=public_domain_by_dates(persons,ev.get('published'))
                for country in JURISDICTIONS:
                    item=rights.get(country) or {}
                    if isinstance(item,dict) and item.get('basis') in ('public-domain','public-domain+licence') and not dates[country]:add(f'witness.rights.{country}: public domain is not established by the recorded dates')
    legacy_work=part(path,'accepted.json') is not None or bool(held) or bool(scope)
    problems.extend(human_page_problems(path,ev,reading,places,second=second,offline=offline))
    problems.extend(original_browser_problems(path,ev,reading,places,second=second,offline=offline))
    for loc,place_evidence in (ev.get('places') or {}).items():
        if place_evidence.get('scan_rebind_pending'):add(f'{loc}: scan binding changed; run kit.py compare and review the page again')
    coverage,debts=language_coverage(path,entry,original,legacy_work,bool(held),second,offline)
    problems.extend(coverage);OWES.extend(debts)
    # the places
    locs=places.get('places') or {}
    if not locs:add('places.json holds no place')
    corrections=places.get('corrections') or []
    for loc,place in locs.items():
        problems.extend(page_number_problems(path,ev,loc,(ev.get('places') or {}).get(loc) or {},offline))
        at=f'place {loc}';text=(place.get('texts') or {}).get(original)
        if loc!=loc.strip() or not loc or (not re.search(r'\S p\d+(-\d+)?[a-z]?$',loc) and not valid_address(place.get('address'))):add(f'{at}: give a division and printed page, or a structured source address')
        if place.get('address') is not None and not valid_address(place['address']):add(f'{at}: structured address needs kind, value and lawful URL')
        for lang in('ru','en'):
            if re.search(r'(pp\.|сс\.)\s*\d+\s*$',str((place.get('label') or {}).get(lang,''))) and not re.search(r'\d\s*[–-]\s*\d+\s*$',str(place['label'][lang])):add(f'{at}: label.{lang} names one page: «p. 226», «с. 226»')
        if held and loc in w['fragments']['places'].get(work,{}) and place!=w['fragments']['places'][work][loc]:add(f'{at}: the bonsai already holds a place of this name')
        if not all((place.get('label') or {}).get(lang) for lang in('ru','en')):add(f'{at}: label wants ru and en (division and printed page)')
        if locator_only:
            if any(value for value in (place.get('texts') or {}).values()):add(f'{at}: locator-only place cannot contain text')
            problems.extend(locator_problems(loc,place,ev))
            continue
        if list(place.get('texts') or {})!=[original] or not isinstance(text,str):add(f'{at}: texts holds the original only, under {original}');continue
        witness_rights=((entry.get('witnesses') or {}).get(original) or {}).get('rights') or {}
        if any(isinstance(witness_rights.get(country),dict) and witness_rights[country].get('basis')=='locator-only' for country in JURISDICTIONS):add(f'{at}: text cannot be distributed while a jurisdiction permits a locator only')
        quoted=any(isinstance(witness_rights.get(country),dict) and witness_rights[country].get('basis')=='short-quotation' for country in JURISDICTIONS)
        for problem in sentences(text,limit=None if quoted else LONGEST,language=original):add(f'{at}: {problem}')
        if quoted:
            problems.extend(quotation_problems(loc,text,(ev.get('places',{}).get(loc) or {}).get('quotation'),ev,second=second,path=path,offline=offline))
        if not str((place.get('links') or {}).get(original,'')).startswith('https://'):add(f'{at}: links.{original} wants the address of the page of the scan')
        if not any(isinstance(ref,list) and len(ref)==2 and ref[0]==original and ref[1] for ref in place.get('refs') or []):add(f'{at}: refs wants [["{original}", "edition, printed page"]]')
    predecessors={loc:place.get('texts',{}).get(original,'') for loc,place in locs.items()}
    for receipt in (part(path,'accepted.json') or {}).get('text_correction_reviews') or []:
        if not any(c.get('review') and c.get('place')==f"{work} {receipt.get('loc')}" and page_number_digest(c)==receipt.get('sha256') for c in corrections):add(f"{receipt.get('loc')}: accepted text correction review has no matching correction")
    for correction in reversed(corrections):
        loc=str(correction.get('place','')).removeprefix(work+' ')
        if correction.get('review') and loc in predecessors:
            before=reverse_reviewed_correction(predecessors[loc],correction)
            if before is not None:predecessors[loc]=before
            else:add(f'{loc}: reviewed correction offset does not match the current text')
    previous_texts=dict(predecessors)
    for correction in reversed(corrections):
        loc=str(correction.get('place','')).removeprefix(work+' ')
        if loc not in locs or not all(correction.get(name) for name in('lang','from','to','evidence')):add(f'correction {correction.get("place")}: lang, from, to, evidence and an existing place are wanted')
        elif not correction.get('review') and correction['to'] not in previous_texts[loc]:add(f'correction {correction["place"]}: «{correction["to"]}» is not in the place')
        elif correction['from']==correction['to']:add(f'correction {correction["place"]}: it changes nothing; take it out of corrections')
        elif any(scan.get('long_s') for scan in scans(ev).values()) and correction['from'].replace('ß','ss')==correction['to'].replace('ß','ss'):add(f'correction {correction["place"]}: «{correction["from"]}» → «{correction["to"]}» only trades ß and ss. In a print with the long s, ſs is the print’s ß and stays ß; ſſ is ss. Put the text back with kit.py cut and take this out of corrections')
        review=correction.get('review')
        if review:
            receipt=(part(path,'accepted.json') or {}).get('text_correction_reviews') or []
            if not any(item.get('loc')==loc and item.get('sha256')==page_number_digest(correction) for item in receipt):add(f'{loc}: text correction is not bound to the accepted packet review')
            for item in receipt:
                if item.get('loc')==loc and item.get('sha256')==page_number_digest(correction) and item.get('previous_sha256') and item['previous_sha256']!=page_number_digest({k:v for k,v in correction.items() if k!='offset'}):add(f'{loc}: prior correction review digest does not match')
            if not review.get('by') or not review.get('second') or review.get('by')==review.get('second') or not review.get('second_model') or len(str(review.get('assessment','')).strip())<30:add(f'{loc}: text correction needs a distinct second reader and assessment')
            scan=scan_of(ev,(ev.get('places') or {}).get(loc));images=(ev.get('places') or {}).get(loc,{}).get('images') or []
            if not any(review.get('url')==f"https://archive.org/download/{scan.get('item')}/page/n{n}.jpg" for n in images) or not re.fullmatch(r'[a-f0-9]{64}',str(review.get('image_sha256',''))) or review.get('file')!=f"text-correction-{review.get('image_sha256')}.img":add(f'{loc}: text correction needs its exact scan page and digest')
            if not offline:
                try:
                    raw=Path(cache(path,review['file'])).read_bytes()
                    if hashlib.sha256(raw).hexdigest()!=review.get('image_sha256') or not valid_page_image(raw):add(f'{loc}: text correction image changed or is invalid')
                except (OSError,KeyError):add(f'{loc}: text correction image is unavailable')
    for correction in corrections:
        loc=str(correction.get('place','')).removeprefix(work+' ')
        if loc in predecessors and not correction.get('review') and correction.get('to') and correction.get('from'):
            predecessors[loc]=predecessors[loc].replace(correction['to'],correction['from'],1)
    # a letter changed against the transcription, which the OCR does not read either, is read on the enlarged line
    for loc,what in by_eye(places,ev):
        if loc in locs and loc not in enlarged(ev,'maker'):owe(f'correction {work} {loc}: {what} rests on the eye alone, for the transcription reads otherwise and the OCR of the page does not read it either. Look at the line at the size of the scan: kit.py image . N --loc "{loc}" --zoom TOP-BOTTOM (the leaf of the place; the band of the page height in percent that holds the line). Keep the correction where the enlarged line bears it letter by letter; where it does not, put the word of the transcription back (kit.py cut) and take the correction out')
    if not locator_only:
        # the evidence, recomputed from the fetched files
        problems.extend(derived_source_problems(ev.get('sources') or [],lambda name:cache(path,name),offline))
        problems.extend(derived_image_problems(ev.get('sources') or [],ev.get('images') or [],lambda name:cache(path,name),offline,legacy=part(path,'accepted.json') is not None))
        scan=ev.get('scan') or {};taken=shots(ev,'maker');live=not offline and bool(ev.get('sources')) and all(os.path.exists(cache(path,source['file'])) for source in ev['sources'])
        ocr_files={source['file'] for source in ev.get('sources',[]) if source['role']=='scan-ocr'}
        all_from_scan=bool(locs) and all((ev.get('places',{}).get(loc) or {}).get('from_scan') and (ev['places'][loc].get('transcription') in ocr_files) for loc in locs)
        if not any(source['role']=='transcription' for source in ev.get('sources',[])) and not all_from_scan:add('no transcription was fetched (kit.py fetch … transcription URL)')
        for source in ev.get('sources',[]):
            if source['role']=='scan-ocr' and any(entry.get('from_scan') for entry in (ev.get('places') or {}).values()) and len(str(source.get('why','')))<60:
                add(f"places are cut from the scan’s own OCR: say with kit.py why {source['file']} \"…\" which proofreading projects were searched on their own sites, for what words, and that none holds the passage")
            if source['role']=='transcription' and source['file'] in {entry.get('transcription') for entry in (ev.get('places') or {}).values()} and not proofread(source['url']) and len(str(source.get('why','')))<60:
                add(f"the words are cut from {source['url']}, which no proofreading project keeps: cut them from Wikisource, Project Gutenberg, the Deutsches Textarchiv, Zeno, Gallica or the like, or say with kit.py why {source['file']} \"…\" what was looked at and why none of them gives this text")
        if not scan.get('item'):add('no scan is named (kit.py ia)')
        if live:
            for source in ev.get('sources',[]):
                file=cache(path,source['file'])
                if not os.path.exists(file) or hashlib.sha256(open(file,'rb').read()).hexdigest()!=source['sha256']:add(f"source {source['file']}: the fetched file is missing or changed")
        if live and not problems:
            fresh=compare(path,ev,places,original);unfixed=set()
            for loc,place in locs.items():
                at=f'place {loc}';mine=ev['places'].get(loc) or {};now=fresh.get(loc) or {};text=place['texts'][original]
                scan=scan_of(ev,mine if mine.get('scan') or mine.get('from_scan') else now);offset=scan.get('image_offset');fetched=taken.get(scan.get('item'),set())
                if scan.get('ocr') and offset is None and scan['item'] not in unfixed:unfixed.add(scan['item']);add(f"{scan['item']}: the image offset is not fixed (look at one image of this scan, compare it with kit.py leaf, then kit.py offset)")
                try:_,source=transcription(path,ev,mine.get('transcription'))
                except SystemExit:add(f'{at}: no transcription is named for it');continue
                back=predecessors[loc]
                problem=in_transcription(back,modern_russian(source) if original=='ru' else source,mine.get('dropped'))
                if problem:add(f'{at}: {problem}')
                strip=lambda entry:[(d['transcription'],d['ocr'],d['leaf']) for d in entry.get('differences',[])]
                if now.get('method')=='ocr':
                    if mine.get('method')!='ocr' or strip(mine)!=strip(now) or mine.get('leaves')!=now.get('leaves') or mine.get('scan',now.get('scan'))!=now.get('scan') or (mine.get('page_number_corrections') and mine.get('printed')!=now.get('printed')):add(f'{at}: the comparison on file is not the comparison of this text (run kit.py compare)');continue
                    for n,d in enumerate(mine['differences'],1):
                        if d.get('stands') not in('transcription','print'):add(f'{at}: difference {n} («{d["transcription"]}» | «{d["ocr"]}») is not judged on the page image')
                        elif d['stands']=='print':add(f'{at}: difference {n} says the print stands: change the text with kit.py correct, then compare again')
                        if offset is not None and d['leaf']+offset not in fetched:add(f'{at}: difference {n} is on leaf {d["leaf"]}; its image n{d["leaf"]+offset} was not fetched')
                    if offset is not None:
                        if not any(n+offset in fetched for n in mine['leaves']) and (mine['differences'] or mine['printed'][0] in(None,'')):add(f'{at}: no image of its page was fetched (the Archive gives no printed page number for leaf {mine["leaves"][0]})')
                        want=f"https://archive.org/details/{scan['item']}/page/n{mine['leaves'][0]+offset}/mode/1up"
                        if place['links'].get(original)!=want:add(f'{at}: links.{original} should be {want}')
                else:
                    if (mine.get('whole') or {}).get('stands')!='transcription':add(f'{at}: the OCR does not hold it; read it whole on the page image and record judge {loc} whole transcription "what you read" (or correct the text)')
                    if not mine.get('images') or not set(mine.get('images',[]))<=fetched:add(f'{at}: fetch the image of every page it runs over with image N --loc "{loc}"')
                    elif now.get('leaves') and offset is not None and not {n+offset for n in now['leaves']}<=set(mine['images']):add(f"{at}: it runs over leaves {now['leaves']}; the images n{', n'.join(str(n+offset) for n in now['leaves'])} are wanted with --loc \"{loc}\"")
    # the reading
    ids=set();cited=set();covered=set();of_reading={}
    conceptions=reading.get('conceptions') or [];links=reading.get('links') or [];left=reading.get('left') or []
    mine_keys={r['key']:r for r in t['readings']};all_keys={r['key']:r for r in w['readings'] if r['work']==work}
    reader=reading.get('reader') or {}
    for name in('id','model','date','reads'):
        if not reader.get(name):add(f'reader.{name} is wanted')
    shelf=((reading.get('library') or {}).get('works') or {}).get(work) or {}
    if shelf.get('original')!=original or shelf.get('places')!='fragments' or shelf.get('tree')!=f'tos.work.{work}' or not all((shelf.get('title') or {}).get(lang) for lang in('ru','en')):add(f'library.works.{work} wants title ru/en, original {original}, tree tos.work.{work}, places fragments')
    if not conceptions:add('the reading holds no conception')
    for c in conceptions:
        at=f"conception {c.get('id')}";cid=str(c.get('id',''))
        if not re.fullmatch(re.escape(work)+r':[a-z0-9]+(-[a-z0-9]+)*',cid) or cid in ids:add(f'{at}: the name wants the form {work}:short-english-name, once')
        ids.add(cid)
        if any(record['id']==cid for record in w['field']['conceptions']):add(f'{at}: the field already holds a conception of this name')
        if c.get('work')!=work:add(f'{at}: work')
        if not c.get('of') or any(key_ not in w['concepts'] for key_ in c.get('of',[])):add(f'{at}: of wants concept keys of the layer (the keys of the task)')
        if not c.get('areas') or any(area not in w['areas'] for area in c.get('areas',[])):add(f'{at}: areas wants area ids (AE01 …)')
        if not c.get('locus'):add(f'{at}: locus')
        formula=[f.get('place') for f in c.get('formula') or []]
        if not formula or any(loc not in locs for loc in formula):add(f'{at}: formula wants places of this packet: [{{"place": "…"}}]')
        cited.update(formula)
        if not all(len(str((c.get('thesis') or {}).get(lang,'')))>=30 for lang in('ru','en')):add(f'{at}: thesis wants ru and en')
        if (c.get('status'),c.get('verdict')) not in(('text','confirmed'),('text','revise'),('reconstruction','reconstruction')):add(f'{at}: status/verdict is text/confirmed, text/revise or reconstruction/reconstruction')
        if c.get('verdict')!='confirmed' and len(str(c.get('note','')))<20:add(f'{at}: a {c.get("verdict")} wants a note saying what changed against the register')
        grounds=c.get('from') or []
        if scope and any(key_ not in scope['keys'] for key_ in grounds):add(f'{at}: source is outside the extension scope')
        if not grounds or any(key_ not in all_keys for key_ in grounds):add(f'{at}: from wants register readings of this work (the keys of the task); nothing is planted that no register read')
        covered.update(grounds)
        for key_ in grounds:of_reading.setdefault(key_,set()).update(c.get('of',[]))
        texts=' '.join(locs[loc]['texts'].get(original,'') for loc in formula if loc in locs)
        style(at+' thesis',c.get('thesis'),texts,original,problems,c.get('negation'))
        if c.get('distinction'):style(at+' distinction',c.get('distinction'),texts,original,problems,c.get('negation'))
    for c in conceptions:
        if c.get('verdict')=='confirmed':
            for key_ in c.get('from') or []:
                missing=[w['canon'].get(k,k) for k in (all_keys.get(key_) or {}).get('concepts',[]) if w['canon'].get(k,k) not in of_reading.get(key_,set())]
                if missing:add(f"conception {c.get('id')}: confirmed, yet the reading {key_} also stands under {', '.join(missing)}: put them under of, or give the verdict revise with a note")
    for loc in locs:
        if loc not in cited:add(f'place {loc}: no conception cites it')
    for n,c in enumerate(conceptions):
        mine=[f.get('place') for f in c.get('formula') or [] if f.get('place') in locs]
        for a in mine:
            for b in mine:
                if a!=b and locs[a]['texts'].get(original,'') and locs[a]['texts'][original] in locs[b]['texts'].get(original,''):add(f"conception {c.get('id')}: its place {a} stands whole inside its place {b}; cite the one that says the thing")
        for other in conceptions[:n]:
            if set(mine) and set(mine)==set(f.get('place') for f in other.get('formula') or []) and difflib.SequenceMatcher(None,str((c.get('thesis') or {}).get('ru','')),str((other.get('thesis') or {}).get('ru',''))).ratio()>0.6:
                add(f"conceptions {other.get('id')} and {c.get('id')} say one thing on the same places: make them one conception grown from both readings")
    link_keys={l['key']:l for l in w['links'] if work in (l.get('works') or [])};known_ids=ids|{record['id'] for record in w['field']['conceptions']};link_ids=set()
    for l in links:
        at=f"link {l.get('id')}"
        if not l.get('id') or l['id'] in link_ids or any(record['id']==l['id'] for record in w['field']['links']):add(f'{at}: the name is missing or taken')
        link_ids.add(l.get('id'))
        for end in('from_concept','to_concept'):
            if l.get(end) not in w['concepts']:add(f'{at}: {end} wants a concept key of the layer')
        if not all((l.get('verb') or {}).get(lang) for lang in('ru','en')):add(f'{at}: verb wants ru and en')
        if not l.get('grounds') or any(ground not in known_ids for ground in l.get('grounds',[])):add(f'{at}: grounds wants conceptions of this packet or of the field')
        if (l.get('status'),l.get('verdict')) not in(('text','confirmed'),('text','revise'),('reconstruction','reconstruction')):add(f'{at}: status/verdict')
        if l.get('verdict')!='confirmed' and len(str(l.get('note','')))<20:add(f'{at}: a {l.get("verdict")} wants a note')
        if scope and any(key_ not in scope['keys'] for key_ in l.get('from',[])):add(f'{at}: source is outside the extension scope')
        if not l.get('from') or any(key_ not in link_keys for key_ in l.get('from',[])):add(f'{at}: from wants register links of this work (the keys of the task)')
        covered.update(l.get('from') or [])
        grounds_text=' '.join(locs[f['place']]['texts'].get(original,'') for c in conceptions if c.get('id') in (l.get('grounds') or []) for f in c.get('formula',[]) if f.get('place') in locs)
        if grounds_text:style(at+' verb',l.get('verb'),grounds_text,original,problems)
    for item in left:
        if item.get('key') not in mine_keys and item.get('key') not in link_keys or len(str(item.get('reason','')))<30:add(f"left {item.get('key')}: a waiting key of this work and a reason in a full sentence are wanted")
        covered.add(item.get('key'))
    for key_ in list(mine_keys)+[l['key'] for l in t['links']]:
        if key_ not in covered:add(f'{key_} is not accounted for: grow a conception or a link from it, or put it under left with the reason')
    # the planting itself, tried dry
    if not problems and not offline:
        run=subprocess.run(['node',os.path.join(REVIEW,'plant.mjs'),'--dry','--packet',packet_selector(path)],capture_output=True,text=True)
        if run.returncode and thrown(run.stderr):add(LINE_FAULT.format(error=thrown(run.stderr)))
        elif run.returncode:problems.extend(f'plant: {line}' for line in (run.stdout+run.stderr).strip().splitlines()[-12:])
    # An admitted language is part of this packet's gate, bound to the exact reviewed file.
    directory=os.path.join(path,'witnesses')
    if os.path.isdir(directory):
        for name in sorted(os.listdir(directory)):
            if not name.endswith('.json') or name[:-5] in ('ru','en','es','de','zh-Hans') or not os.path.isfile(os.path.join(directory,name+'.accepted')):continue
            file=os.path.join(directory,name);raw=open(file,'rb').read();receipt=load(file+'.accepted')
            if receipt.get('sha256')!=hashlib.sha256(raw).hexdigest():add(f'{name}: witness changed since acceptance');continue
            for problem in witness_problems(path,name[:-5],json.loads(raw),second=True,offline=offline):add(f'{name}: {problem}')
    problems.extend(reopen_problems(path,second=second))
    if second and not problems:problems.extend(second_gate(path,reading,ev,locs,owe,locator_only=locator_only))
    return problems,'open' if not problems else 'shut'

def second_gate(path,reading,ev,locs,owe=None,locator_only=False):
    problems=[];add=problems.append;review=part(path,'review.json');owe=owe or add
    if review is None:return ['review.json is missing']
    for loc,what in by_eye(part(path,'places.json') or {},ev):
        if loc in locs and loc not in enlarged(ev,'second'):owe(f'review: the correction {what} at {loc} rests on the eye alone. Read the line at the size of the scan — kit.py image . N --by second --loc "{loc}" --zoom TOP-BOTTOM — and say under review.images what the enlarged line reads')
    reader=review.get('reader') or {}
    if not all(reader.get(name) for name in('id','model','date')) or reader.get('id')==reading['reader']['id']:add('review.reader wants id, model, date, and is another reader than the maker')
    for kind in('conceptions','links'):
        said=review.get(kind) or {}
        for record in reading.get(kind) or []:
            mine=said.get(record['id']) or {}
            if mine.get('says') not in('yes','partly','no'):add(f"review.{kind}.{record['id']}: says wants yes, partly or no")
            elif mine['says']!='yes' and len(str(mine.get('note','')))<30:add(f"review.{kind}.{record['id']}: a {mine['says']} wants a note with the words of the place it rests on")
    waits=[item['key'] for item in reading.get('left') or [] if not item.get('disputed')];answers=review.get('left') or {}
    for key_ in waits:
        mine=answers.get(key_) or {}
        if not isinstance(mine.get('holds'),bool):add(f'review.left.{key_}: holds wants true (the work does not bear the reading, as the reason says) or false (it does)')
        elif not mine['holds'] and len(str(mine.get('note','')))<30:add(f'review.left.{key_}: holds false wants a note naming the place that bears the reading (division, page, opening words)')
    if locator_only:
        for loc in locs:
            answer=(review.get('locators') or {}).get(loc) or {}
            if len(str(answer.get('reads','')))<30 or answer.get('url')!=(ev.get('places',{}).get(loc,{}).get('locator') or {}).get('url'):
                add(f'review.locators: {loc} needs the second reader’s account of reading the same book')
    else:
        seen=shots(ev,'second');looked=[item for item in review.get('images') or [] if item.get('loc') in locs]   # a page looked at for a place that has since left the packet asks nothing
        problems.extend(pdf_page_review_problems(ev,locs,looked))
        need=min(len(locs),max(3,math.ceil(len(locs)*0.3)))
        whole=[loc for loc,entry in ev['places'].items() if entry.get('method')=='image' and loc in locs]     # a place that has left the packet stays in the evidence and asks nothing
        if len({item.get('loc') for item in looked})<need:add(f'review.images: the second reader looks at the pages of at least {need} places; {len(looked)} are recorded')
        for loc,entry in ev['places'].items():
            quote=entry.get('quotation') or {}
            if loc not in locs or not quote:continue
            read=[item for item in looked if item.get('loc')==loc and item.get('reads')=='as the packet']
            maker=next((image for image in ev.get('images',[]) if image.get('by','maker')=='maker' and image.get('sha256')==quote.get('sha256')),None)
            match=second_witness_image({'images':ev.get('images') or []},{'url':maker.get('url'),'image_sha256':quote.get('sha256')},reader.get('id')) if maker else None
            images=[match] if match else []
            if not any(any(image.get('n')==item.get('n') and image.get('item')==scan_of(ev,entry).get('item') for image in images) for item in read):add(f'review.images: {loc} quotation must be read by the second reader on the same page')
        for loc in whole:
            if loc not in {item.get('loc') for item in looked}:add(f'review.images: {loc} was read from the image alone and wants the second reader’s look')
            entry=ev['places'][loc]
            scan=scan_of(ev,entry);offset=scan.get('image_offset');fetched=seen.get(scan.get('item'),set())
            if entry.get('from_scan') and entry.get('leaves') and offset is not None:
                needed={n+offset for n in entry['leaves']}
                read={item.get('n') for item in looked if item.get('loc')==loc and item.get('n') in fetched and item.get('reads') in ('as the packet','differs')}
                if not needed<=read:add(f'review.images: {loc} was cut from the scan OCR and wants the second reader’s look at every page: '+', '.join('n'+str(n) for n in sorted(needed-read)))
        for item in looked:
            scan=scan_of(ev,ev['places'].get(item.get('loc')))
            if item.get('loc') not in locs or item.get('n') not in seen.get(scan.get('item'),set()) or item.get('reads') not in('as the packet','differs') or (item.get('reads')=='differs' and len(str(item.get('note','')))<20):add(f"review.images {item.get('loc')}: loc, n (fetched with image N --by second), reads ('as the packet' or 'differs' with a note)")
    # The review answers for the packet as its reader saw it: what was settled out of the reading since still counts against accept.
    clean=all(answer.get('says')=='yes' for kind in('conceptions','links') for answer in (review.get(kind) or {}).values()) and all(item.get('reads')=='as the packet' for item in review.get('images') or []) and all((answers.get(key_) or {}).get('holds') is True for key_ in waits) and not reopen_unresolved(path)
    if review.get('accept') is not clean:add(f'review.accept must be {str(clean).lower()}: it is true only when every conception and link is a yes, every page reads as the packet and every left reading is rightly left')
    return problems

def cmd_gate(args):
    path=packet(args.packet);problems,state=gate(path,second=args.second,offline=args.offline)
    save(cache(path,'gate.json'),{'state':state,'second':args.second,'problems':problems,'owes':list(OWES),'at':datetime.datetime.now().isoformat(timespec='seconds')})
    for problem in problems:print('- '+problem)
    for debt in OWES:print('owes: '+debt)
    print(f'gate: {state}'+(f' ({len(problems)} to settle)' if problems else '')+(f', owes {len(OWES)}' if OWES else ''))
    sys.exit(3 if any(problem.startswith('line: ') for problem in problems) else 1 if problems else 0)     # 3: the line's fault, not the packet's

def cmd_ready(args):
    fault=ready()
    if fault:print(f'line: the checkout cannot plant — {fault}\nAfter main gains corpus texts: node bonsai/lib/layers.mjs fetch access/public-corpus');sys.exit(3)
    print('ready')

def cmd_settle(args):
    """What the second reader does not confirm leaves the reading: the conceptions and links go under left, and so do places nothing cites any more."""
    path=packet(args.packet);reading,review,places,ev=part(path,'reading.json'),part(path,'review.json') or die('no review.json'),part(path,'places.json'),evidence(path)
    out=lambda kind:{record['id'] for record in reading[kind] if (review.get(kind) or {}).get(record['id'],{}).get('says')!='yes'}
    gone=out('conceptions');moved=[]
    for record in reading['conceptions']:
        if record['id'] in gone:
            for key_ in record['from']:moved.append({'key':key_,'reason':f"The second reader ({review['reader']['id']}) does not confirm {record['id']}: {review['conceptions'].get(record['id'],{}).get('note','no note')}",'disputed':record})
    reading['conceptions']=[record for record in reading['conceptions'] if record['id'] not in gone];ids={record['id'] for record in reading['conceptions']}
    for record in list(reading['links']):
        if record['id'] in out('links') or not set(record['grounds'])&ids and any(ground in gone for ground in record['grounds']):
            reading['links'].remove(record)
            for key_ in record['from']:moved.append({'key':key_,'reason':f"The second reader ({review['reader']['id']}) does not confirm {record['id']}, or its ground is disputed: {(review.get('links') or {}).get(record['id'],{}).get('note','its conception left the reading')}",'disputed':record})
        else:record['grounds']=[ground for ground in record['grounds'] if ground not in gone]
    kept={key_ for record in reading['conceptions'] for key_ in record['from']}|{key_ for record in reading['links'] for key_ in record['from']}
    # What was not confirmed stays on record under left, also where a kept conception grows from the same reading: a later reader takes it up from there.
    reading['left']=[item for item in reading.get('left',[]) if item.get('disputed') or item['key'] not in kept]+moved
    cited={f['place'] for record in reading['conceptions'] for f in record['formula']}
    for loc in [loc for loc in places['places'] if loc not in cited]:
        del places['places'][loc];ev['places'].pop(loc,None);places['corrections']=[c for c in places['corrections'] if c['place']!=f"{places['work']} {loc}"]
    for name,value in(('reading.json',reading),('places.json',places),('evidence.json',ev)):save(os.path.join(path,name),value)
    print(f"settled: {len(gone)} conceptions left the reading; {len(reading['conceptions'])} conceptions, {len(reading['links'])} links, {len(places['places'])} places stay")

def witness_conflict(path,language,data):
    """Only disjoint independently reviewed places may be combined."""
    work=work_of(path);current=Path(witness_file(path,language));directory=Path(path)
    parent=directory.parent.parent if directory.parent.name=='extensions' else directory
    candidates=[parent/'witnesses'/f'{language}.json']
    extensions=parent/'extensions'
    if extensions.is_dir():
        candidates.extend(extension/'witnesses'/f'{language}.json' for extension in extensions.iterdir()
                          if extension.is_dir() and ((extension/'accepted.json').is_file() or (part(str(extension),'scope.json') or {}).get('kind')=='witness-only'))
    known=[]
    for file in candidates:
        receipt=Path(str(file)+'.accepted')
        if file==current or not receipt.is_file():continue
        try:
            raw=file.read_bytes();signed=load(receipt)
            if signed.get('sha256')!=hashlib.sha256(raw).hexdigest():return f'{language}: existing witness changed since acceptance'
            known.append(json.loads(raw))
        except (OSError,ValueError,TypeError):return f'{language}: existing witness evidence is invalid'
    for previous in known:
        if set(previous.get('places') or {}) & set(data.get('places') or {}):return f'{language}: an existing witness already covers one of these places'
    fragments=world()['fragments'];work_entry=(fragments.get('works') or {}).get(work) or {}
    if language in (work_entry.get('witnesses') or {}):
        record=(((fragments.get('provenance') or {}).get('translation_packets') or {}).get(work) or {}).get(language) or {}
        if not record or (not known and not str(record.get('record','')).endswith(f"/{packet_selector(path)}/witnesses/{language}.json")):
            return f'{language}: an existing witness cannot be extended without its reviewed source packet'
    return None

def cmd_accept(args):
    path=packet(args.packet);problems,state=gate(path,second=True,offline=args.offline)
    if problems:
        for problem in problems:print('- '+problem)
        die('the gate is shut: nothing is accepted')
    if reopen_unresolved(path):die('an unresolved reopening finding cannot be accepted')
    if any(item.get('reads')!='as the packet' for item in ((part(path,'review.json') or {}).get('images') or [])):die('a page that differs from the packet cannot be accepted')
    # Bind every new witness only after the online second gate has passed.
    directory=os.path.join(path,'witnesses')
    if os.path.isdir(directory):
        pending=[os.path.join(directory,name) for name in os.listdir(directory) if name.endswith('.json') and not os.path.isfile(os.path.join(directory,name+'.accepted'))]
        if pending and args.offline:die('accepting a new witness requires the online second gate')
        for file in pending:
            data=load(file);conflict=witness_conflict(path,Path(file).stem,data)
            if conflict:die(conflict)
            issues=witness_problems(path,Path(file).stem,data,second=True,offline=False)
            if issues:die('witness is not ready: '+'; '.join(issues))
        for file in pending:save(file+'.accepted',{'sha256':hashlib.sha256(Path(file).read_bytes()).hexdigest(),'date':today()})
    review=part(path,'review.json');reading=part(path,'reading.json');ev=evidence(path)
    used={entry.get('transcription') for entry in ev['places'].values()}
    places=part(path,'places.json') or {};entry=places.get('entry') or {};original=(entry.get('witnesses') or {}).get(entry.get('original')) or {}
    licence_source=((original.get('licence_evidence') or {}).get('source') or {}).get('file')
    if licence_source:used.add(licence_source)
    first=next((source['file'] for source in ev['sources'] if source['role']=='transcription'),None)
    ev['sources']=[source for source in ev['sources'] if source['role']!='transcription' or source['file'] in used or (None in used and source['file']==first)];save(os.path.join(path,'evidence.json'),ev)   # a place that names no transcription was cut from the first
    basis=args.basis or f"Made by {reading['reader']['id']}; every conception and link read a second time by {review['reader']['id']}, who confirms each; the gate recomputed the comparison with the scan."
    prior=part(path,'accepted.json') or {}
    save(os.path.join(path,'accepted.json'),{'by':args.by,'date':today(),'basis':basis,'conceptions':len(reading['conceptions']),'links':len(reading['links']),'left':len(reading.get('left',[])),**({'reopen_sha256':page_number_digest({'history':part(path,'reopen-history.json',[]),'resolutions':part(path,'reopen-resolutions.json',{})})} if part(path,'reopen-history.json',[]) else {}),**{key:prior[key] for key in ('page_number_reviews','text_correction_reviews') if prior.get(key)}})
    print(f'accepted: {work_of(path)}')

def reopen_reader_scope(value):
    # The line stamps model and effort after each gate; these are execution metadata,
    # not a change to the maker's or second reader's substantive answers.
    value=json.loads(json.dumps(value)) if value is not None else None
    if isinstance(value,dict) and isinstance(value.get('reader'),dict):
        value['reader'].pop('model',None);value['reader'].pop('effort',None)
    return value

def reopen_scope(path,record):
    return page_number_digest({'finding':record.get('finding'),'reader':record.get('reader'),'reading':reopen_reader_scope(part(path,'reading.json')),'places':part(path,'places.json')})

def reopen_problems(path,second=False):
    history=part(path,'reopen-history.json',[]);resolutions=part(path,'reopen-resolutions.json',{})
    if not isinstance(history,list) or not isinstance(resolutions,dict):return ['reopen: invalid finding or resolution history']
    problems=[];review=part(path,'review.json') or {};maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id')
    accepted=part(path,'accepted.json')
    if history and accepted is not None and accepted.get('reopen_sha256')!=page_number_digest({'history':history,'resolutions':resolutions}):problems.append('reopen: the acceptance must bind the exact finding and independent resolutions')
    for index,record in enumerate(history,1):
        if not isinstance(record,dict) or not record.get('finding'):problems.append(f'reopen {index}: invalid finding');continue
        key=page_number_digest(record);response=resolutions.get(key) or {};scope=reopen_scope(path,record)
        if response.get('maker')!=maker or not maker or response.get('scope_sha256')!=scope or len(str(response.get('assessment','')).strip())<60:
            problems.append(f'reopen {index}: maker must address this finding with kit.py reopen-response: {record["finding"]}');continue
        answer=response.get('review') or {}
        if second:
            reader=(review.get('reader') or {}).get('id')
            if not reader or reader==maker or answer.get('reader')!=reader or answer.get('review_sha256')!=page_number_digest(reopen_reader_scope(review)) or type(answer.get('resolved')) is not bool or len(str(answer.get('assessment','')).strip())<60:
                problems.append(f'reopen {index}: independent reader must assess the finding with kit.py reopen-review: {record["finding"]}')
        elif answer.get('resolved') is False:
            problems.append(f'reopen {index}: second reader found the issue unresolved; maker must respond again: {answer.get("assessment", "")}')
    return problems

def reopen_unresolved(path):
    history=part(path,'reopen-history.json',[]);resolutions=part(path,'reopen-resolutions.json',{})
    return any(((resolutions.get(page_number_digest(record)) or {}).get('review') or {}).get('resolved') is False for record in history)

def cmd_reopen_response(args):
    path=packet(args.packet);history=part(path,'reopen-history.json',[])
    if not isinstance(history,list) or not 1<=args.index<=len(history):die('choose a recorded reopen finding by its 1-based index')
    maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id')
    if not maker or args.maker!=maker or len(args.assessment.strip())<60:die('the packet maker must give a specific assessment of at least 60 characters')
    record=history[args.index-1];resolutions=part(path,'reopen-resolutions.json',{})
    if not isinstance(record,dict) or not record.get('finding') or not isinstance(resolutions,dict):die('invalid reopening record')
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its reopening resolutions')
    key=page_number_digest(record);previous=resolutions.get(key) or {};prior=list(previous.get('prior') or [])
    if previous:prior.append({k:v for k,v in previous.items() if k!='prior'})
    resolutions[key]={'maker':maker,'assessment':args.assessment,'scope_sha256':reopen_scope(path,record),'date':today(),**({'prior':prior} if prior else {})}
    save(os.path.join(path,'reopen-resolutions.json'),resolutions);print('finding addressed; independent re-review is required')

def cmd_reopen_review(args):
    path=packet(args.packet);history=part(path,'reopen-history.json',[])
    if not isinstance(history,list) or not 1<=args.index<=len(history):die('choose a recorded reopen finding by its 1-based index')
    record=history[args.index-1];resolutions=part(path,'reopen-resolutions.json',{});response=resolutions.get(page_number_digest(record)) or {}
    review=part(path,'review.json') or {};reader=(review.get('reader') or {}).get('id')
    if part(path,'accepted.json') is not None:die('an accepted packet cannot change its reopening resolutions')
    if response.get('scope_sha256')!=reopen_scope(path,record) or not response.get('maker'):die('the maker must address the current finding first')
    if not reader or args.reader!=reader or reader==response['maker'] or args.resolution not in ('resolved','unresolved') or len(args.assessment.strip())<60:die('the independent packet reader must give a resolution and specific assessment of at least 60 characters')
    response['review']={'reader':reader,'assessment':args.assessment,'resolved':args.resolution=='resolved','review_sha256':page_number_digest(reopen_reader_scope(review)),'date':today()}
    save(os.path.join(path,'reopen-resolutions.json'),resolutions);print('independent finding review recorded for this exact packet reading')

def cmd_reopen(args):
    """Withdraw an unplanted acceptance with its independent finding and predecessor."""
    path=packet(args.packet);work=work_of(path);accepted=part(path,'accepted.json')
    if not accepted:die('an accepted packet is required')
    if accepted.get('page_number_reviews') or accepted.get('text_correction_reviews'):die('the acceptance holds correction receipts; preserve them and use a separately reviewed correction route')
    if packet_selector(path)!=work:die('reopening extensions requires a separately reviewed route')
    w=world();fragments=w['fragments'];reading=part(path,'reading.json') or {};review=part(path,'review.json') or {}
    reader=(review.get('reader') or {}).get('id');maker=(reading.get('reader') or {}).get('id')
    if work in (fragments.get('works') or {}) or work in ((fragments.get('provenance') or {}).get('packets') or []):die('the work is already planted; do not reopen it through this route')
    if any((record.get('reading') or {}).get('reader')==maker for kind in ('conceptions','links') for record in w['field'].get(kind,[])):die('the packet reading is already planted')
    witnesses=Path(path)/'witnesses'
    if witnesses.is_dir() and any(witnesses.glob('*.json.accepted')):die('an accepted witness is attached; preserve it and use a reviewed correction')
    if not reader or reader==maker or args.reader!=reader or len(args.finding.strip())<40:die('the existing independent reader must record a specific finding of at least 40 characters')
    snapshots={name:part(path,name) for name in ('accepted.json','reading.json','review.json','places.json','evidence.json')}
    history=part(path,'reopen-history.json',[])
    if not isinstance(history,list):die('reopen history is invalid')
    history.append({'date':today(),'reader':reader,'finding':args.finding,'predecessor':snapshots})
    save(os.path.join(path,'reopen-history.json'),history)
    os.unlink(os.path.join(path,'accepted.json'))
    save(os.path.join(path,'review.json'),{})
    print('acceptance withdrawn with history; correct the reading and obtain a fresh independent review')

def cmd_status(args):
    rows=[]
    for name in sorted(os.listdir(PACKETS)) if os.path.isdir(PACKETS) else []:
        path=os.path.join(PACKETS,name)
        if not os.path.isdir(path):continue
        reading=part(path,'reading.json') or {};state='accepted' if part(path,'accepted.json') else 'blocked' if part(path,'blocked.json') else (load(cache(path,'gate.json')) if os.path.exists(cache(path,'gate.json')) else {}).get('state','—')
        rows.append(f"{state:9} {name}  conceptions {len(reading.get('conceptions',[]))} links {len(reading.get('links',[]))} left {len(reading.get('left',[]))} review {'yes' if part(path,'review.json') else 'no'}")
    print('\n'.join(rows) or 'no packets')

def old_russian_source(text):
    return bool(re.search(r'[ѣѢіІѳѲѵѴ]|[бвгджзклмнпрстфхцчшщ]ъ\b',text,re.I))

def modern_russian(text,old_source=False):
    """Use the stand's reform rules, preserving the source's old-spelling context for a short cut."""
    script="import {pathToFileURL} from 'node:url'; import {readFileSync} from 'node:fs'; const m=await import(pathToFileURL(process.argv[1])); const t=readFileSync(0,'utf8').replaceAll('ѵ','и').replaceAll('Ѵ','И'); process.stdout.write(process.argv[2]==='old'?m.modernRussian('ѣ '+t).slice(2):m.modernRussian(t));"
    module=os.path.join(ROOT,'access','web','src','public-research','orthography.mjs')
    run=subprocess.run(['node','--input-type=module','-e',script,module,'old' if old_source else 'modern'],input=text,text=True,capture_output=True)
    if run.returncode:die('Russian orthography conversion failed: '+run.stderr)
    return run.stdout

def witness_file(path,lang):
    if not re.fullmatch(r'[a-z]{2,3}(?:-[A-Z][a-z]{3})?',lang):die('language must be a BCP 47 language such as ru or zh-Hans')
    return os.path.join(path,'witnesses',lang+'.json')

def witness_cache(path,lang,name):
    if os.path.basename(name)!=name or name in ('.','..'):die('source file must be a filename')
    directory=cache(path,'witnesses',lang);os.makedirs(directory,exist_ok=True)
    return os.path.join(directory,name)

def witness_data(path,lang):
    file=witness_file(path,lang)
    if not os.path.isfile(file):die(f'no witness {lang}: run witness-open')
    return load(file)

def editable_witness_data(path,lang):
    if os.path.isfile(witness_file(path,lang)+'.accepted'):die('an accepted witness cannot be changed; use a separately reviewed extension for new places')
    return witness_data(path,lang)

def cmd_witness_withdraw(args):
    """Withdraw only an unplanted witness while preserving its exact acceptance."""
    path=packet(args.packet);file=Path(witness_file(path,args.language));marker=Path(str(file)+'.accepted')
    if not file.is_file() or not marker.is_file():die('an accepted witness is required')
    raw=file.read_bytes();receipt_raw=marker.read_bytes();receipt=json.loads(receipt_raw);data=json.loads(raw)
    digest=hashlib.sha256(raw).hexdigest()
    if receipt.get('sha256')!=digest:die('witness bytes differ from the acceptance; preserve and investigate')
    fragments=world()['fragments'];work=work_of(path);lang=args.language
    if lang in (((fragments.get('works') or {}).get(work) or {}).get('witnesses') or {}) or lang in (((fragments.get('provenance') or {}).get('translation_packets') or {}).get(work) or {}):die('this language is already planted; use a separately reviewed correction')
    if any(lang in ((place.get('texts') or {})) for place in (((fragments.get('places') or {}).get(work) or {}).values())):die('this language is already planted; use a separately reviewed correction')
    reader=(data.get('review') or {}).get('reader');maker=(data.get('witness') or {}).get('maker')
    if not reader or reader==maker or args.reader!=reader or len(args.finding.strip())<40:die('the existing independent reader must record a specific finding of at least 40 characters')
    history=Path(path)/'witness-history'/lang/digest
    if history.exists():die('this acceptance already has a withdrawal history; preserve and inspect it')
    history.mkdir(parents=True);(history/'witness.json').write_bytes(raw);(history/'acceptance.json').write_bytes(receipt_raw)
    save(str(history/'withdrawal.json'),{'reader':reader,'finding':args.finding,'witness_sha256':digest,'acceptance_sha256':hashlib.sha256(receipt_raw).hexdigest(),'replace':args.replace,'date':today()})
    if (history/'witness.json').read_bytes()!=raw or (history/'acceptance.json').read_bytes()!=receipt_raw:die('withdrawal history could not be verified')
    marker.unlink()
    if args.replace:file.unlink()
    else:
        data['review']={};save(str(file),data)
    print('unplanted acceptance withdrawn with exact evidence preserved; new independent review is required')

def witness_withdrawal_problems(path,lang):
    root=Path(path)/'witness-history'/lang;problems=[]
    if root.exists() and not root.is_dir():return ['witness withdrawal history is not a directory']
    for history in root.iterdir() if root.exists() else []:
        try:
            if not history.is_dir() or not re.fullmatch(r'[a-f0-9]{64}',history.name):raise ValueError()
            raw=(history/'witness.json').read_bytes();receipt_raw=(history/'acceptance.json').read_bytes();record=load(str(history/'withdrawal.json'));receipt=json.loads(receipt_raw)
            if hashlib.sha256(raw).hexdigest()!=history.name or receipt.get('sha256')!=history.name or record.get('witness_sha256')!=history.name or record.get('acceptance_sha256')!=hashlib.sha256(receipt_raw).hexdigest() or not record.get('reader') or len(str(record.get('finding','')).strip())<40:raise ValueError()
        except (OSError,ValueError,TypeError,AttributeError):problems.append(f'{lang}: witness withdrawal history {history.name} is missing or changed')
    retired=Path(path)/'retired-witnesses'/lang
    if retired.exists() and not retired.is_dir():return problems+['retired witness history is not a directory']
    for history in retired.iterdir() if retired.exists() else []:
        try:
            if not history.is_dir() or not re.fullmatch(r'[a-f0-9]{64}',history.name):raise ValueError()
            raw=(history/'witness.json').read_bytes();record=load(str(history/'retirement.json'));prior=json.loads(raw)
            if hashlib.sha256(raw).hexdigest()!=history.name or record.get('witness_sha256')!=history.name or record.get('maker')!=(prior.get('witness') or {}).get('maker') or not record.get('maker') or len(str(record.get('reason','')).strip())<40:raise ValueError()
        except (OSError,ValueError,TypeError,AttributeError):problems.append(f'{lang}: retired witness history {history.name} is missing or changed')
    return problems

def cmd_witness_retire(args):
    """Retire an unaccepted witness before opening a different edition."""
    path=packet(args.packet);file=Path(witness_file(path,args.language));marker=Path(str(file)+'.accepted')
    if not file.is_file():die('an existing unaccepted witness is required')
    if marker.exists():die('this witness is accepted; use witness-withdraw')
    raw=file.read_bytes();data=json.loads(raw);maker=(data.get('witness') or {}).get('maker')
    if not maker or args.maker!=maker or len(args.reason.strip())<40:die('the witness maker must give a specific retirement reason of at least 40 characters')
    fragments=world()['fragments'];work=work_of(path);lang=args.language
    if lang in (((fragments.get('works') or {}).get(work) or {}).get('witnesses') or {}) or lang in (((fragments.get('provenance') or {}).get('translation_packets') or {}).get(work) or {}):die('this language is already planted; use a separately reviewed correction')
    if any(lang in (place.get('texts') or {}) for place in (((fragments.get('places') or {}).get(work) or {}).values())):die('this language is already planted; use a separately reviewed correction')
    digest=hashlib.sha256(raw).hexdigest();history=Path(path)/'retired-witnesses'/lang/digest
    if history.exists():die('this witness already has retirement history; preserve and inspect it')
    history.mkdir(parents=True);(history/'witness.json').write_bytes(raw)
    save(str(history/'retirement.json'),{'maker':maker,'reason':args.reason,'witness_sha256':digest,'date':today()})
    if (history/'witness.json').read_bytes()!=raw or file.read_bytes()!=raw or marker.exists():die('the witness changed while preserving its retirement history')
    file.unlink();print('unaccepted witness retired with exact prior evidence; a different edition can now be opened')

def language_search_file(path,lang):
    witness_file(path,lang)  # apply the same language validation
    return os.path.join(path,'language-searches',lang+'.json')

def language_search_scope(path):
    return sorted(set(((world()['fragments'].get('places') or {}).get(work_of(path)) or {})) | set(((part(path,'places.json') or {}).get('places') or {})))

def language_search_problems(path,lang,data,second=False):
    problems=[];proof=data.get('search') or {};review=data.get('review') or {}
    if Path(path).parent.name=='extensions':problems.append('record a whole-language search on the parent packet, not an extension')
    if data.get('language')!=lang:problems.append('language differs from filename')
    work=((world()['fragments'].get('works') or {}).get(work_of(path)) or {})
    if not work or lang==work.get('original') or lang in (work.get('witnesses') or {}):problems.append('the language must still be missing from an existing work')
    if not proof.get('maker'):problems.append('the maker must be identified')
    if proof.get('places')!=language_search_scope(path) or not proof.get('places'):problems.append('search must identify every current original place')
    missing=missing_search_sites(proof.get('tried') or [])
    if missing:problems.append('search outcomes missing for '+', '.join(sorted(missing)))
    if len(str(proof.get('reason','')).strip())<30:problems.append('a detailed account of the search outcome is required')
    if second and (not review.get('reader') or review.get('reader')==proof.get('maker') or not review.get('model') or not review.get('date') or review.get('sha256')!=page_number_digest(proof) or len(str(review.get('assessment','')).strip())<40):problems.append('a distinct second reader must assess this exact search')
    return problems

def cmd_language_search(args):
    path=packet(args.packet);file=language_search_file(path,args.language)
    if os.path.exists(file+'.accepted'):die('the accepted search must be withdrawn before replacing it')
    if args.language not in ('ru','en','es','de','zh-Hans'):die('choose an interface language')
    fragments=world()['fragments'];work=(fragments.get('works') or {}).get(work_of(path)) or {}
    if not work or args.language==work.get('original') or args.language in (work.get('witnesses') or {}):die('search is for a missing language of an existing work')
    record=load(args.file)
    if not isinstance(record,dict) or set(record)!={'maker','reason','tried'}:die('search input needs maker, reason and tried')
    data={'language':args.language,'search':{**record,'places':language_search_scope(path)},'review':{}}
    issues=language_search_problems(path,args.language,data)
    if issues:die('search is incomplete: '+'; '.join(issues))
    os.makedirs(os.path.dirname(file),exist_ok=True);save(file,data);print('search recorded; independent review is required')

def cmd_language_search_gate(args):
    path=packet(args.packet);file=language_search_file(path,args.language)
    if not os.path.isfile(file):die('record the language search first')
    data=load(file);issues=language_search_problems(path,args.language,data,second=args.second)
    for issue in issues:print(issue)
    print('gate: '+('shut' if issues else 'open'))
    if issues:sys.exit(1)

def cmd_language_search_review(args):
    path=packet(args.packet);file=language_search_file(path,args.language)
    if os.path.exists(file+'.accepted'):die('the accepted search cannot be changed')
    data=part(os.path.dirname(file),os.path.basename(file)) or die('record the language search first')
    data['review']={'reader':args.reader,'model':args.model,'date':today(),'sha256':page_number_digest(data['search']),'assessment':args.assessment}
    issues=language_search_problems(path,args.language,data,second=True)
    if issues:die('search review is incomplete: '+'; '.join(issues))
    save(file,data);print('independent search assessment recorded')

def cmd_language_search_accept(args):
    path=packet(args.packet);file=language_search_file(path,args.language)
    if os.path.exists(file+'.accepted'):die('search is already accepted')
    raw=Path(file).read_bytes();issues=language_search_problems(path,args.language,json.loads(raw),second=True)
    if issues:die('search is not ready: '+'; '.join(issues))
    save(file+'.accepted',{'sha256':hashlib.sha256(raw).hexdigest(),'date':today()});print('language search accepted')

def cmd_language_search_withdraw(args):
    path=packet(args.packet);file=language_search_file(path,args.language)+'.accepted'
    if not os.path.isfile(file):die('there is no accepted language search')
    os.unlink(file);print('language search withdrawn; the evidence file remains')

MEXICO_LFDA='https://www.diputados.gob.mx/LeyesBiblio/pdf/LFDA.pdf'
def mexico_scope(witness,places):
    return page_number_digest({'witness':{k:v for k,v in witness.items() if k!='mexico_quotation'},'places':places})

def mexico_problems(witness,places,review,maker,second=False,path=None,lang=None,offline=False,require_approval=True):
    proof=witness.get('mexico_quotation') or {};problems=[]
    if not isinstance(proof,dict):return ['Mexico quotation assessment must be an object']
    if proof.get('law_url')!=MEXICO_LFDA or proof.get('scope_sha256')!=mexico_scope(witness,places):problems.append('Mexico quotation assessment needs the official LFDA and the current exact witness and places')
    digest=proof.get('law_sha256');filename=proof.get('law_file')
    if not re.fullmatch(r'[a-f0-9]{64}',str(digest or '')) or filename!=f'mexico-law-{digest}.pdf':problems.append('Mexico quotation assessment needs the digest of the fetched law')
    elif path and not offline:
        try:
            raw=Path(witness_cache(path,lang,filename) if lang else cache(path,filename)).read_bytes()
            if hashlib.sha256(raw).hexdigest()!=digest or not raw.startswith(b'%PDF-'):problems.append('Mexico law source changed or is invalid')
        except OSError:problems.append('Mexico law source is unavailable')
    for field in ('non_substantial','normal_exploitation','source_integrity'):
        if len(str(proof.get(field,'')).strip())<40:problems.append(f'Mexico quotation assessment needs a detailed {field} account')
    if second:
        check=(review or {}).get('mexico_quotation') or {};reader=(review or {}).get('reader');reader=reader.get('id') if isinstance(reader,dict) else reader
        if not reader or reader==maker or check.get('reader')!=reader or not check.get('date') or check.get('sha256')!=page_number_digest(proof) or len(str(check.get('assessment','')).strip())<60:problems.append('Mexico quotation assessment needs a distinct second reader and dated review of the same evidence')
        if check.get('outcome') not in ('approved','rejected','pending'):problems.append('Mexico quotation assessment needs an explicit review outcome')
        elif require_approval and check['outcome']!='approved':problems.append(f"Mexico quotation assessment is {check['outcome']}; independent approval is required")
    return problems

def mexico_context(path,lang):
    if lang:
        data=witness_data(path,lang);return data,data.get('witness') or {},data.get('places') or {},data.get('review') or {},(data.get('witness') or {}).get('maker')
    places=part(path,'places.json') or {};entry=places.get('entry') or {}
    if entry:
        data=places;original=entry.get('original');witness=(entry.get('witnesses') or {}).get(original) or die('no original witness')
    else:
        data=evidence(path);entry=((world()['fragments'].get('works') or {}).get(work_of(path)) or {});original=entry.get('original')
        witness=dict((entry.get('witnesses') or {}).get(original) or die('no existing original witness'))
        witness.pop('mexico_quotation',None)
        if data.get('mexico_quotation'):witness['mexico_quotation']=data['mexico_quotation']
    review=part(path,'review.json') or {};maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id')
    return data,witness,places.get('places') or {},review,maker

def cmd_mexico_quotation(args):
    path=packet(args.packet)
    if args.language:editable_witness_data(path,args.language)
    elif part(path,'accepted.json'):die('an accepted packet needs a separately reviewed correction')
    data,witness,places,review,maker=mexico_context(path,args.language)
    if not maker or not places:die('record the maker and exact places first')
    raw=Path(args.law_file).read_bytes() if getattr(args,'law_file',None) else get(MEXICO_LFDA)
    if not raw.startswith(b'%PDF-'):die('the official law source did not yield a PDF')
    digest=hashlib.sha256(raw).hexdigest();filename=f'mexico-law-{digest}.pdf'
    Path(witness_cache(path,args.language,filename) if args.language else cache(path,filename)).write_bytes(raw)
    proof={'law_url':MEXICO_LFDA,'law_file':filename,'law_sha256':digest,'scope_sha256':mexico_scope(witness,places),'non_substantial':args.non_substantial,'normal_exploitation':args.normal_exploitation,'source_integrity':args.source_integrity}
    witness['mexico_quotation']=proof;review.pop('mexico_quotation',None)
    issues=mexico_problems(witness,places,review,maker,path=path,lang=args.language)
    if issues:die('Mexico quotation assessment is incomplete: '+'; '.join(issues))
    if args.language:data['review']=review;save(witness_file(path,args.language),data)
    else:
        if (part(path,'places.json') or {}).get('entry'):save(os.path.join(path,'places.json'),data)
        else:data['mexico_quotation']=proof;save(os.path.join(path,'evidence.json'),data)
        if review:save(os.path.join(path,'review.json'),review)
    print('Mexico quotation assessment recorded; independent review required')

def cmd_mexico_quotation_review(args):
    path=packet(args.packet)
    if args.language:editable_witness_data(path,args.language)
    elif part(path,'accepted.json'):die('an accepted packet needs a separately reviewed correction')
    data,witness,places,review,maker=mexico_context(path,args.language);reader=review.get('reader');reader=reader.get('id') if isinstance(reader,dict) else reader
    if not reader or reader==maker:die('record a different second reader first')
    if args.outcome not in ('approved','rejected','pending'):die('choose an explicit Mexico quotation review outcome')
    proof=witness.get('mexico_quotation') or {};prior=review.get('mexico_quotation')
    if prior:review.setdefault('mexico_quotation_history',[]).append(prior)
    review['mexico_quotation']={'reader':reader,'date':today(),'sha256':page_number_digest(proof),'assessment':args.assessment,'outcome':args.outcome}
    issues=mexico_problems(witness,places,review,maker,second=True,path=path,lang=args.language,require_approval=False)
    if issues:die('Mexico quotation review is incomplete: '+'; '.join(issues))
    if args.language:data['review']=review;save(witness_file(path,args.language),data)
    else:save(os.path.join(path,'review.json'),review)
    print(f'independent Mexico quotation review recorded: {args.outcome}')

def cmd_extension_open(args):
    parent=packet(args.packet);work=work_of(parent)
    if not part(parent,'accepted.json'):die('an extension requires an accepted parent packet')
    if not re.fullmatch(r'[a-z0-9]+(?:-[a-z0-9]+)*',args.name):die('extension name must be a short lowercase slug')
    task_=task(world(),work);available={item['key'] for item in task_['readings']+task_['links']}
    keys=[key.strip() for key in args.keys.split(',') if key.strip()]
    if not keys or len(set(keys))!=len(keys) or any(key not in available for key in keys):die('choose distinct waiting reading or link keys from kit task')
    path=os.path.join(parent,'extensions',args.name)
    if os.path.exists(path):die('extension already exists')
    os.makedirs(path);original=task_['already']['original'];known=world()['works'][work]
    title={key:known['title'].get(key,'') for key in ('ru','en')}
    save(os.path.join(path,'scope.json'),{'keys':keys})
    save(os.path.join(path,'places.json'),{'work':work,'places':{},'corrections':[],'checked_on_the_page_image':[]})
    save(os.path.join(path,'reading.json'),{'reader':{'id':f'terra-{today()}-{args.name}','model':'gpt-6.1-terra','date':today(),'reads':''},'note':'',
        'library':{'agents':{a['id']:{'tree':f"tos.agent.{a['id']}"} for a in task_['authors']},'works':{work:{'title':title,'original':original,'tree':f'tos.work.{work}','places':'fragments'}}},
        'conceptions':[],'links':[],'left':[]})
    save(os.path.join(path,'evidence.json'),evidence(path))
    print(path);show(scoped_task(path,task_))

def cmd_witness_extension_open(args):
    parent=packet(args.packet);work=work_of(parent)
    if Path(parent).parent.name=='extensions' or work not in (world()['fragments'].get('works') or {}):die('a witness-only extension requires an existing planted work and its parent packet')
    if not re.fullmatch(r'[a-z0-9]+(?:-[a-z0-9]+)*',args.name):die('extension name must be a short lowercase slug')
    path=os.path.join(parent,'extensions',args.name)
    if os.path.exists(path):die('extension already exists')
    os.makedirs(path);save(os.path.join(path,'scope.json'),{'kind':'witness-only','work':work})
    save(os.path.join(path,'places.json'),{'work':work,'places':{},'corrections':[],'checked_on_the_page_image':[]})
    print(path+'\nopen and independently review each witness; no original reading is created')

def cmd_import(args):
    """Bring reviewed working data into a new packet; the gate remains the authority."""
    path=packet(args.packet)
    if part(path,'accepted.json') is not None:die('accepted packets cannot be replaced; use a separately reviewed extension')
    if args.part not in ('places','reading','review','blocked'):die('this packet part cannot be imported')
    value=load(args.file)
    if not isinstance(value,dict):die('the imported JSON must be an object')
    if args.part=='places' and value.get('work')!=work_of(path):die('places.json names another work')
    if args.part=='blocked' and value.get('ground') not in GROUNDS:die('blocked.json has an unknown ground')
    save(os.path.join(path,args.part+'.json'),value)
    print(f'{args.part}.json imported; run the relevant gate')

def cmd_unblock(args):
    path=packet(args.packet)
    if part(path,'accepted.json') is not None:die('accepted packet cannot be unblocked')
    file=os.path.join(path,'blocked.json')
    if not os.path.isfile(file):die('no block is recorded')
    os.unlink(file);print('block cleared; make the packet and run its gate')

def cmd_witness_parent(args):
    work=args.work;w=world()
    if work not in w['fragments']['works']:die('the work must already be in fragments.json')
    path=os.path.join(PACKETS,work);os.makedirs(path,exist_ok=True)
    print(path)

def cmd_witness_open(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.exists(file):die('witness already exists')
    metadata=load(args.metadata)
    if not isinstance(metadata,dict):die('metadata must be a JSON object')
    os.makedirs(os.path.dirname(file),exist_ok=True)
    save(file,{'language':args.language,'witness':metadata,'sources':[],'images':[],'places':{},'review':{}})
    print(file)

def cmd_witness_info(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);info=load(args.file)
    fields={'edition','year','url','licence','attribution','maker','normalization'}
    if not isinstance(info,dict) or not set(info)<=fields:die('witness info has an unknown or protected field')
    data['witness'].update(info);data['review']={};save(witness_file(path,args.language),data)
    print('witness information updated; a new second reading is required')

def cmd_witness_rights(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    data['witness'].setdefault('rights',{})[args.jurisdiction]={'basis':args.basis,'law':args.law}
    data['review']={};save(witness_file(path,args.language),data)
    print('jurisdictional rights updated; a new second reading is required')

LICENCE_NAMES={'creativecommons.org/licenses/by/4.0':'CC BY 4.0','creativecommons.org/licenses/by-sa/4.0':'CC BY-SA 4.0','creativecommons.org/licenses/by-nc/4.0':'CC BY-NC 4.0',
               'creativecommons.org/licenses/by-nc-sa/4.0':'CC BY-NC-SA 4.0','creativecommons.org/licenses/by-nd/4.0':'CC BY-ND 4.0','creativecommons.org/licenses/by-nc-nd/4.0':'CC BY-NC-ND 4.0',
               'creativecommons.org/publicdomain/zero/1.0':'CC0 1.0'}
def pdf_layer(raw,first=1,last=3):
    """The text layer of the first pages of a PDF, read by pdftotext; empty when there is none."""
    try:return subprocess.run(['pdftotext','-f',str(first),'-l',str(last),'-layout','-','-'],input=raw,capture_output=True,timeout=60).stdout.decode('utf-8',errors='replace')
    except (OSError,subprocess.SubprocessError):return ''

def source_names_licence(raw,url):
    if url.encode() in raw or url.replace('https://','http://',1).encode() in raw:return True
    if not raw.startswith(b'%PDF'):return False
    text=re.sub(r'\s+',' ',pdf_layer(raw)).casefold();key=re.sub(r'^https?://|/+$','',url.casefold())
    name=LICENCE_NAMES.get(key)
    return key in text or bool(name and re.search(re.escape(name.casefold()).replace(r'\ ',r'[\s-]*'),text))

def license_problems(witness,sources,local_file,review,maker,second=False,offline=False):
    rights=witness.get('rights') or {}
    if not isinstance(rights,dict):return ['licensed basis requires a rights object']
    if not any(isinstance(rights.get(country),dict) and rights[country].get('basis')=='licensed' for country in JURISDICTIONS):return []
    problems=[];proof=witness.get('licence_evidence') or {};url=proof.get('licence_url','');source=proof.get('source') or {}
    if not lawful_url(url) or len(str(proof.get('explanation','')).strip())<40 or proof.get('edition')!=witness.get('edition'):
        problems.append('licensed basis needs its licence URL, exact edition and an explanation of coverage')
    known=next((item for item in sources if item.get('file')==source.get('file') and item.get('url')==source.get('url') and item.get('sha256')==source.get('sha256')),None)
    if not known or not lawful_url(source.get('url','')) or not re.fullmatch(r'[a-f0-9]{64}',str(source.get('sha256',''))):problems.append('licensed basis needs the fetched source and its SHA-256')
    elif not offline:
        try:
            raw=Path(local_file(source['file'])).read_bytes()
            if hashlib.sha256(raw).hexdigest()!=source['sha256'] or not source_names_licence(raw,url):problems.append('licensed source changed or does not name that licence URL')
        except (OSError,KeyError,TypeError):problems.append('licensed source is unavailable')
    if second:
        checked=review.get('licence') or {}
        if checked.get('source_sha256')!=source.get('sha256') or checked.get('licence_url')!=url or len(str(checked.get('assessment','')).strip())<40 or not checked.get('reader') or checked.get('reader')==maker:
            problems.append('licensed basis requires an independent review of the same edition and licence evidence')
        reader=review.get('reader') or {}
        reader_id=reader.get('id') if isinstance(reader,dict) else reader
        if checked.get('reader')!=reader_id:problems.append('licence reviewer must be the packet second reader')
    return problems

def cmd_licence(args):
    path=packet(args.packet)
    if args.language:
        data=editable_witness_data(path,args.language);witness=data['witness'];sources=data['sources'];local=lambda name:witness_cache(path,args.language,name)
    else:
        places=part(path,'places.json') or {};entry=places.get('entry') or {};witness=(entry.get('witnesses') or {}).get(entry.get('original')) or die('no original witness')
        sources=evidence(path).get('sources') or [];local=lambda name:cache(path,name)
    source=next((item for item in sources if item.get('file')==args.source),None)
    if not source or not lawful_url(args.url):die('fetch the exact edition source and provide a lawful licence URL')
    raw=Path(local(args.source)).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=source.get('sha256') or not source_names_licence(raw,args.url):die('source changed or does not contain the licence URL')
    if len(args.explanation.strip())<40:die('explain how this licence covers this exact edition')
    witness['licence_evidence']={'edition':witness.get('edition'),'licence_url':args.url,'source':{key:source[key] for key in ('file','url','sha256')},'explanation':args.explanation}
    if args.language:
        data['review']={};save(witness_file(path,args.language),data)
    else:
        save(os.path.join(path,'places.json'),places)
        review=part(path,'review.json')
        if review is not None:review.pop('licence',None);save(os.path.join(path,'review.json'),review)
    print('licence evidence recorded; an independent review is required')

def cmd_licence_review(args):
    path=packet(args.packet)
    if len(args.assessment.strip())<40:die('explain the second reader’s assessment of this edition and licence')
    if args.language:
        data=editable_witness_data(path,args.language);witness=data['witness'];review=data.setdefault('review',{});reader=review.get('reader');maker=witness.get('maker')
    else:
        places=part(path,'places.json') or {};entry=places.get('entry') or {};witness=(entry.get('witnesses') or {}).get(entry.get('original')) or die('no original witness')
        review=part(path,'review.json') or die('review.json is missing');reader=(review.get('reader') or {}).get('id');maker=((part(path,'reading.json') or {}).get('reader') or {}).get('id')
    if not reader or reader==maker:die('first record a different second reader')
    proof=witness.get('licence_evidence') or {};source=proof.get('source') or {}
    if not source.get('sha256') or not proof.get('licence_url'):die('record licence evidence first')
    review['licence']={'reader':reader,'source_sha256':source['sha256'],'licence_url':proof['licence_url'],'assessment':args.assessment}
    if args.language:save(witness_file(path,args.language),data)
    else:save(os.path.join(path,'review.json'),review)
    print('independent licence assessment recorded')

def cmd_witness_place_withdraw(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed; use a separately reviewed correction')
    data=witness_data(path,args.language);places=data.get('places') or {}
    if args.loc not in places:die('no such place in this witness')
    if len(args.reason.strip())<40:die('explain the reason for withdrawal in a full sentence')
    place=places.pop(args.loc)
    data.setdefault('withdrawn_places',[]).append({'loc':args.loc,'place':place,'review':data.get('review') or {},'reason':args.reason,'date':today()})
    (data.get('missing_places') or {}).pop(args.loc,None)
    data['review']={};save(file,data)
    print('place withdrawn with its evidence and review; document the gap and renew the independent reading')

def cmd_witness_missing(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    original={**((world()['fragments'].get('places') or {}).get(work_of(path),{})),**((part(path,'places.json') or {}).get('places') or {})}
    if args.loc not in original:die('no original place with that location')
    if args.loc in (data.get('places') or {}):die('the witness already provides this place')
    record=load(args.file);issues=missing_language_problems(record)
    if issues:die('missing-place search is incomplete: '+'; '.join(issues))
    data.setdefault('missing_places',{})[args.loc]=record;data['review']={}
    save(witness_file(path,args.language),data);print(f'{args.loc}: searched language gap recorded; renew the second review')

def cmd_witness_candidate(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);raw=Path(args.file).read_bytes();candidate=json.loads(raw)
    if not isinstance(candidate,dict) or candidate.get('work')!=work_of(path):die('candidate names another work')
    if candidate.get('language') not in (None,args.language):die('candidate names another language')
    name='research-candidate.json';Path(witness_cache(path,args.language,name)).write_bytes(raw)
    data['candidate']={'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'status':candidate.get('status')};data['review']={};save(witness_file(path,args.language),data)
    print('research candidate recorded; source and page evidence must still pass the gate')

def cmd_witness_cut_candidate(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    ref=data.get('candidate') or {}
    if not ref.get('file'):die('record a research candidate first')
    raw=Path(witness_cache(path,args.language,ref['file'])).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=ref.get('sha256'):die('research candidate changed')
    candidate=(json.loads(raw).get('places') or {}).get(args.loc) or {}
    text=candidate.get('text')
    if not isinstance(text,str) or not text.strip():die('candidate has no text for this place')
    args.open=text;args.close=text
    cmd_witness_cut(args)

def cmd_witness_person(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if not re.fullmatch(r'Q[1-9]\d*',args.qid):die('person must be a Wikidata QID')
    url=f'https://www.wikidata.org/wiki/Special:EntityData/{args.qid}.json';raw=get(url)
    entity=json.loads(raw)['entities'][args.qid]
    year=lambda prop:entity_year(entity,prop)
    died=year('P570')
    labels=entity.get('labels',{});label=lambda lang:(labels.get(lang) or {}).get('value')
    name={'en':label('en') or label('mul') or label('ru') or args.qid,'ru':label('ru') or label('en') or label('mul') or args.qid}
    filename=f'wikidata-{args.qid}-{hashlib.sha256(raw).hexdigest()}.json'
    with open(witness_cache(path,args.language,filename),'wb') as file:file.write(raw)
    person={'role':args.role,'qid':args.qid,'name':name,'born':year('P569'),'died':died,'source':f'https://www.wikidata.org/wiki/{args.qid}','evidence':{'file':filename,'sha256':hashlib.sha256(raw).hexdigest(),'url':url}}
    if died is None and getattr(args,'bounded',False):
        bound=death_bound(entity)
        if bound is None:die(f'{args.qid}: no claim bounds the death before {BOUND_BEFORE}')
        person.update({'died_before':bound['year'],'death_bound':bound})
    people=data['witness'].setdefault('persons',[])
    matches=[p for p in people if isinstance(p,dict) and p.get('qid')==args.qid and p.get('role')==args.role]
    if len(matches)>1:die('multiple records have this role and QID; review them before replacing either')
    if matches:
        for key in ('wartime','wartime_source','rehabilitated','rehabilitation_source'):
            if key in matches[0]:person[key]=matches[0][key]
    for prior in people:
        if isinstance(prior,dict) and prior.get('qid')==args.qid and prior.get('role')==args.role and prior!=person:
            data.setdefault('person_corrections',[]).append({'removed':prior,'replacement':person,'reason':'Refetched this same Wikidata participant; the prior source and record remain available.','date':today()})
    data['witness']['persons']=[p for p in people if not(isinstance(p,dict) and p.get('qid')==args.qid and p.get('role')==args.role)]+[person]
    data['review']={};save(witness_file(path,args.language),data)
    print(f"{name['en']} ({args.role}): {person['born']}–{died}, {args.qid}")

def cmd_witness_person_remove(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed; retain its acceptance and seek a reviewed correction')
    data=witness_data(path,args.language);people=data['witness'].get('persons') or []
    removed=[p for p in people if isinstance(p,dict) and p.get('role')==args.role and p.get('qid')==args.qid]
    if len(removed)!=1:die('exactly one person with that role and QID must be present')
    if len(args.reason.strip())<30:die('give the reason in a full sentence')
    data['witness']['persons']=[p for p in people if p is not removed[0]]
    data.setdefault('person_corrections',[]).append({'removed':removed[0],'reason':args.reason,'date':today()})
    data['review']={};save(file,data)
    print('person removed with history; independent review must be renewed')

def cmd_witness_person_remove_source(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed; retain its acceptance and seek a reviewed correction')
    data=witness_data(path,args.language);people=data['witness'].get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and p.get('role')==args.role and not p.get('qid') and isinstance(p.get('name'),dict) and p['name'].get('en')==args.name and isinstance(p.get('evidence'),dict) and p['evidence'].get('kind')=='reference']
    if len(matches)!=1:die('exactly one source-backed participant without a QID must match that role and name')
    if len(args.reason.strip())<30:die('give the reason in a full sentence')
    data['witness']['persons']=[p for p in people if p is not matches[0]]
    data.setdefault('person_corrections',[]).append({'removed':matches[0],'reason':args.reason,'date':today()})
    data['review']={};save(file,data);print('source-backed person removed with history; independent review must be renewed')

def cmd_witness_person_remove_legacy(args):
    """Remove one exact unsupported string, retaining it and the reason for review."""
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed; retain its acceptance and seek a reviewed correction')
    data=witness_data(path,args.language);people=data['witness'].get('persons') or []
    matches=[i for i,p in enumerate(people) if isinstance(p,str) and p==args.name]
    if len(matches)!=1:die('exactly one matching legacy string must be present')
    if len(args.reason.strip())<30:die('give the reason in a full sentence')
    removed=people.pop(matches[0]);data.setdefault('person_corrections',[]).append({'removed':removed,'reason':args.reason,'date':today()})
    data['review']={};save(file,data)
    print('legacy person removed with history; independent review must be renewed')

def publisher_identity_problems(path,lang,data,person,second=False,offline=False):
    proof=person.get('evidence') or {};url=proof.get('url','');isbn=proof.get('isbn','');name=(person.get('name') or {}).get('en','')
    sources=data.get('sources') or [];witness=data.get('witness') or {}
    digits=lambda text:re.sub(r'[^0-9Xx]','',str(text))
    host=urllib.parse.urlsplit(url).hostname or ''
    if person.get('role')!='translator' or person.get('qid') or person.get('born') is not None or person.get('died') is not None or person.get('died_before') is not None or host.lower()!='ast.ru' or not lawful_url(url) or not re.fullmatch(r'[0-9]{13}',isbn) or isbn not in digits(witness.get('edition','')) or not any(all(source.get(k)==proof.get(k) for k in ('file','url','sha256')) for source in sources):return ['publisher identity needs its exact edition ISBN, fetched publisher card and an undated translator']
    if not offline:
        try:
            raw=Path(witness_cache(path,lang,proof['file'])).read_bytes();content=plain(raw,proof['file'])
            if hashlib.sha256(raw).hexdigest()!=proof.get('sha256') or not name or not re.search(r'Переводчик\s*:\s*'+re.escape(name)+r'(?=\s|$)',content) or isbn not in digits(content):return ['publisher card changed or does not name this translator and ISBN']
        except (OSError,KeyError,TypeError):return ['publisher identity source is missing']
    if second:
        review=data.get('review') or {};check=(review.get('publisher_identities') or {}).get(page_number_digest(person)) or {};reader=review.get('reader')
        if not reader or reader==witness.get('maker') or check.get('reader')!=reader or len(str(check.get('assessment','')).strip())<60:return ['publisher identity needs the independent witness reader assessment']
    return []

def cmd_witness_person_publisher(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);people=data['witness'].get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and p.get('role')=='translator' and not p.get('qid') and isinstance(p.get('name'),dict) and p['name'].get('en')==args.old_name]
    if len(matches)!=1 or len(args.reason.strip())<40:die('choose exactly one non-QID translator and explain the identity correction')
    old=matches[0];source=next((item for item in data.get('sources') or [] if item.get('file')==args.source),None)
    if not source:die('fetch the publisher card as a witness source first')
    if any(old.get(key) is not None for key in ('born','died','died_before','wartime','rehabilitated')):die('retain separately sourced dates and legal status; this route only replaces an undated identity')
    isbn=re.sub(r'[^0-9Xx]','',args.isbn);person={'role':'translator','name':{'en':args.full_name,'ru':args.full_name},'born':None,'died':None,'source':source.get('url'),'evidence':{'kind':'publisher-identity',**{k:source.get(k) for k in ('file','url','sha256')},'isbn':isbn}}
    issues=publisher_identity_problems(path,args.language,data,person)
    if issues:die('; '.join(issues))
    data['witness']['persons']=[person if p is old else p for p in people]
    data.setdefault('person_corrections',[]).append({'removed':old,'replacement':person,'reason':args.reason,'date':today()})
    data['review']={};save(file,data);print('publisher identity recorded with previous participant retained; independent review required')

def cmd_witness_publisher_review(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);review=data.get('review') or {};people=(data.get('witness') or {}).get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and isinstance(p.get('name'),dict) and p['name'].get('en')==args.name and isinstance(p.get('evidence'),dict) and p['evidence'].get('kind')=='publisher-identity']
    if len(matches)!=1 or not review.get('places') or not review.get('reader') or review['reader']!=args.reader or args.reader==(data.get('witness') or {}).get('maker') or len(args.assessment.strip())<60:die('the independent witness reader must assess this exact publisher identity')
    issues=publisher_identity_problems(path,args.language,data,matches[0])
    if issues:die('; '.join(issues))
    review.setdefault('publisher_identities',{})[page_number_digest(matches[0])]={'reader':args.reader,'assessment':args.assessment}
    data['review']=review;save(file,data);print('independent publisher identity assessment recorded')

def image_identity_problems(path,lang,data,person,second=False,offline=False):
    proof=person.get('evidence') or {};images=data.get('images') or [];witness=data.get('witness') or {}
    if not isinstance(proof,dict):return ['invalid printed participant evidence']
    page=next((i for i in images if isinstance(i,dict) and i.get('by')=='maker' and all(i.get(k)==proof.get(k) for k in ('file','url','sha256'))),None)
    name=person.get('name') or {}
    if not page or not lawful_url(proof.get('url')) or person.get('source')!=proof.get('url') or person.get('qid') or person.get('born') is not None or person.get('died_before') is not None or type(person.get('died')) is not int or not isinstance(name,dict) or not name.get('en') or len(str(proof.get('wording','')).strip())<8 or len(str(proof.get('assessment','')).strip())<60 or not proof.get('page'):
        return ['printed participant needs the exact maker image, visible name and year, page wording and assessment']
    if not offline:
        try:
            raw=Path(witness_cache(path,lang,proof['file'])).read_bytes()
            if hashlib.sha256(raw).hexdigest()!=proof.get('sha256') or not valid_page_image(raw):return ['printed participant page changed']
        except (OSError,KeyError,TypeError):return ['printed participant page is unavailable']
    if second:
        review=data.get('review') or {};reader=review.get('reader');check=(review.get('image_identities') or {}).get(page_number_digest(person)) or {}
        image=second_witness_image(data,{'url':proof['url'],'image_sha256':proof['sha256']},reader)
        if not reader or reader==witness.get('maker') or not image or check.get('reader')!=reader or len(str(check.get('assessment','')).strip())<60:return ['printed participant needs independent page and identity assessment']
        if not offline:
            try:
                raw=Path(witness_cache(path,lang,image['file'])).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=image.get('sha256'):return ['second participant page changed']
            except (OSError,KeyError,TypeError):return ['second participant page is unavailable']
    return []

def cmd_witness_person_image(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);people=data['witness'].get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and p.get('role')==args.role and not p.get('qid') and isinstance(p.get('name'),dict) and p['name'].get('en')==args.old_name]
    image=next((i for i in data.get('images') or [] if i.get('file')==args.image and i.get('by')=='maker'),None)
    if len(matches)!=1 or not image or len(args.reason.strip())<40:die('select one non-QID participant and maker page, and explain the correction')
    old=matches[0]
    if old.get('evidence') or any(old.get(key) is not None for key in ('born','died_before','wartime','rehabilitated')) or old.get('died') not in (None,args.died):die('retain separately sourced identity, dates and legal status; this route replaces only an unsupported provisional participant')
    person={'role':args.role,'name':{'en':args.name,'ru':args.name},'born':None,'died':args.died,'source':image['url'],'evidence':{'kind':'image-identity',**{k:image.get(k) for k in ('file','url','sha256')},'page':args.page,'wording':args.wording,'assessment':args.assessment}}
    issues=image_identity_problems(path,args.language,data,person)
    if issues:die('; '.join(issues))
    data['witness']['persons']=[person if p is old else p for p in people];data.setdefault('person_corrections',[]).append({'removed':old,'replacement':person,'reason':args.reason,'date':today()});data['review']={}
    save(file,data);print('printed participant recorded with prior identity retained; independent page assessment required')

def cmd_witness_image_identity_review(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);review=data.get('review') or {};reader=review.get('reader');people=(data.get('witness') or {}).get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and (p.get('name') or {}).get('en')==args.name and (p.get('evidence') or {}).get('kind')=='image-identity']
    if len(matches)!=1 or not review.get('places') or not reader or reader!=args.reader or reader==(data.get('witness') or {}).get('maker') or len(args.assessment.strip())<60:die('the independent witness reader must assess this exact printed participant')
    person=matches[0];proof=person['evidence']
    if image_identity_problems(path,args.language,data,person) or not second_witness_image(data,{'url':proof['url'],'image_sha256':proof['sha256']},reader):die('record and inspect the same participant page as second reader first')
    review.setdefault('image_identities',{})[page_number_digest(person)]={'reader':reader,'assessment':args.assessment};data['review']=review;save(file,data);print('independent printed participant assessment recorded')

def creator_identity_problems(path,lang,data,person,offline=False):
    proof=person.get('evidence') or {};witness=data.get('witness') or {};licence=witness.get('licence_evidence') or {};source=licence.get('source') or {};names=person.get('name');name=names.get('en') if isinstance(names,dict) else None
    if person.get('role')!='translator' or person.get('qid') or person.get('born') is not None or person.get('died') is not None or person.get('died_before') is not None or not name or not lawful_url(licence.get('licence_url')) or person.get('source')!=source.get('url') or not any(all(item.get(k)==source.get(k) for k in ('file','url','sha256')) for item in data.get('sources') or []) or any(proof.get(k)!=source.get(k) for k in ('file','url','sha256')):return ['creator identity needs the exact fetched licence declaration and an unknown date']
    if not offline:
        try:
            raw=Path(witness_cache(path,lang,proof['file'])).read_bytes();content=(pdf_layer(raw) if raw.startswith(b'%PDF-') else plain(raw,proof['file'])).casefold()
            if hashlib.sha256(raw).hexdigest()!=proof.get('sha256') or name.casefold() not in content or not source_names_licence(raw,licence.get('licence_url','')):return ['creator declaration changed or does not identify the creator and licence']
        except (OSError,KeyError,TypeError):return ['creator declaration is unavailable']
    return []

def cmd_witness_creator(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);witness=data.get('witness') or {};source=(witness.get('licence_evidence') or {}).get('source') or {}
    if any(isinstance(p,dict) and p.get('role')=='translator' for p in witness.get('persons') or []):die('the witness already names a translator; retain or correct that record first')
    person={'role':'translator','name':{'en':args.name,'ru':args.name},'born':None,'died':None,'source':source.get('url'),'evidence':{'kind':'creator-declaration',**{k:source.get(k) for k in ('file','url','sha256')}}}
    issues=creator_identity_problems(path,args.language,data,person)
    if issues:die('; '.join(issues))
    witness.setdefault('persons',[]).append(person);data['review']={};save(witness_file(path,args.language),data);print('creator recorded from their licence declaration; independent authority review required')

def cmd_witness_person_source(args):
    path=packet(args.packet);data=witness_data(path,args.language)
    if os.path.isfile(witness_file(path,args.language)+'.accepted'):die('an accepted witness cannot be changed')
    if not trusted(args.url):die('person reference must be on a trusted catalogue or reference site')
    bound=getattr(args,'died_before',None);note=getattr(args,'bound_note',None)
    if getattr(args,'anonymous',False) and args.role!='author':die('--anonymous here identifies an unknown original author, not a translator or editor')
    if bound is not None:
        if args.died is not None or bound>BOUND_BEFORE or len(str(note or '').strip())<30:die('a sourced ancient death bound needs --died-before at most 1800, a detailed --bound-note, and no exact death year')
    elif getattr(args,'anonymous',False):die('--anonymous needs a sourced ancient death bound')
    raw=Path(args.file).read_bytes() if args.file else get(args.url)
    text=plain(raw,args.name+'.html').casefold()
    if args.name.casefold() not in text:die('the reference does not contain the stated name')
    for year in (args.born,args.died):
        if year is not None and str(year) not in text:die(f'the reference does not contain year {year}')
    filename='person-'+hashlib.sha256(raw).hexdigest()+'.html'
    Path(witness_cache(path,args.language,filename)).write_bytes(raw)
    person={'role':args.role,'name':{'en':args.name,'ru':args.ru or args.name},'born':args.born,'died':args.died,'source':args.url,
            'evidence':{'kind':'reference','file':filename,'sha256':hashlib.sha256(raw).hexdigest(),'url':args.url}}
    if any((urllib.parse.urlsplit(args.url).hostname or '').endswith('.'+host) or urllib.parse.urlsplit(args.url).hostname==host for host in ('ci.nii.ac.jp','cardiffnaturalists.org.uk')):
        person['evidence']['identity_review_required']=True
    if bound is not None:
        person.update({'died_before':bound,'death_bound':{'year':bound,'from':args.url,'note':note}})
        if getattr(args,'anonymous',False):person['anonymous']=True
    people=data['witness'].setdefault('persons',[])
    matches=[p for p in people if isinstance(p,dict) and p.get('name')==person['name'] and p.get('role')==args.role]
    if len(matches)>1:die('multiple records have this role and name; review them before replacing either')
    if matches:
        for key in ('wartime','wartime_source','rehabilitated','rehabilitation_source'):
            if key in matches[0]:person[key]=matches[0][key]
        if matches[0]!=person:data.setdefault('person_corrections',[]).append({'removed':matches[0],'replacement':person,'reason':'Updated the source for this same named participant; the prior source and record remain available.','date':today()})
    data['witness']['persons']=[p for p in people if not(isinstance(p,dict) and p.get('name')==person['name'] and p.get('role')==args.role)]+[person]
    data['review']={};save(witness_file(path,args.language),data)
    print(f'{args.name} ({args.role}): source saved')

def reference_identity_problems(data,person,second=False):
    witness=data.get('witness') or {};proof=person.get('evidence') or {}
    if second and proof.get('identity_review_required'):
        assessment=((data.get('review') or {}).get('reference_identities') or {}).get(page_number_digest({'person':person,'edition':witness.get('edition')})) or {}
        if assessment.get('reader')!=(data.get('review') or {}).get('reader') or not assessment.get('reader') or assessment.get('reader')==witness.get('maker') or len(str(assessment.get('assessment','')).strip())<60:return ['reference participant requires independent source and edition identity assessment']
    return []

def cmd_witness_reference_review(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);review=data.get('review') or {};reader=review.get('reader');witness=data.get('witness') or {}
    matches=[p for p in witness.get('persons') or [] if isinstance(p,dict) and p.get('role')==args.role and (p.get('name') or {}).get('en')==args.name and (p.get('evidence') or {}).get('identity_review_required')]
    if len(matches)!=1 or not reader or reader!=args.reader or reader==witness.get('maker') or not review.get('places') or len(args.assessment.strip())<60:die('the independent witness reader must assess this exact participant, reference and edition after page review')
    person=matches[0];proof=person['evidence']
    try:
        raw=Path(witness_cache(path,args.language,proof['file'])).read_bytes()
        if hashlib.sha256(raw).hexdigest()!=proof.get('sha256') or args.name.casefold() not in plain(raw,proof['file']).casefold():die('the participant reference changed or lacks the name')
    except (OSError,KeyError,TypeError):die('the participant reference is unavailable')
    review.setdefault('reference_identities',{})[page_number_digest({'person':person,'edition':witness.get('edition')})]={'reader':reader,'assessment':args.assessment}
    data['review']=review;save(witness_file(path,args.language),data);print('independent source and edition identity assessment recorded')

def witness_linked_bound_problems(path,lang,person,offline=False):
    proof=person.get('bound_reference')
    if not proof:
        bound=person.get('death_bound') or {}
        if person.get('qid') and person.get('died_before') is not None and isinstance(bound,dict) and bound.get('from')=='wikidata':
            evidence_record=person.get('evidence') or {};qid=person.get('qid')
            if person.get('died') is not None or type(person.get('died_before')) is not int or person['died_before']>BOUND_BEFORE or bound.get('semantics')!='exclusive' or bound.get('year')!=person.get('died_before') or evidence_record.get('url')!=f'https://www.wikidata.org/wiki/Special:EntityData/{qid}.json' or not re.fullmatch(r'[a-f0-9]{64}',str(evidence_record.get('sha256',''))):return ['Wikidata bound needs exclusive-year semantics and its exact fetched entity; renew the review']
            if not offline:
                try:
                    raw=Path(witness_cache(path,lang,evidence_record['file'])).read_bytes();source=json.loads(raw)['entities'][qid]
                    if hashlib.sha256(raw).hexdigest()!=evidence_record['sha256'] or death_bound(source)!=bound:return ['Wikidata bound or its source entity changed']
                except (OSError,KeyError,ValueError,TypeError):return ['Wikidata bound source entity is unavailable']
            return []
        if 'bound_reference' in person or (person.get('qid') and person.get('died_before') is not None and (not isinstance(person.get('death_bound'),dict) or person['death_bound'].get('from')!='wikidata')):return ['linked person bound needs its exact reference']
        return []
    if not isinstance(proof,dict):return ['invalid linked person bound evidence']
    bound=person.get('death_bound') or {};qid=person.get('qid');url=proof.get('url');evidence_record=person.get('evidence') or {}
    if not isinstance(evidence_record,dict) or not isinstance(bound,dict) or type(person.get('died_before')) is not int or person['died_before']>BOUND_BEFORE or person.get('died') is not None or not re.fullmatch(r'Q[1-9]\d*',str(qid or '')) or not trusted(url) or bound.get('from')!=url or bound.get('year')!=person.get('died_before') or len(str(bound.get('note','')).strip())<30 or not proof.get('name') or not re.fullmatch(r'[a-f0-9]{64}',str(proof.get('sha256',''))) or proof.get('file')!=f"person-bound-{proof.get('sha256')}.html" or evidence_record.get('url')!=f'https://www.wikidata.org/wiki/Special:EntityData/{qid}.json':return ['linked person bound needs the exact Wikidata entity, reference and assessment']
    if not offline:
        try:
            raw=Path(witness_cache(path,lang,proof['file'])).read_bytes();entity_raw=Path(witness_cache(path,lang,evidence_record['file'])).read_bytes();entity=json.loads(entity_raw)['entities'][qid]
            links=[urllib.parse.unquote(str(v.get('url'))).rstrip('/') for v in (entity.get('sitelinks') or {}).values()]
            if hashlib.sha256(raw).hexdigest()!=proof['sha256'] or hashlib.sha256(entity_raw).hexdigest()!=evidence_record['sha256'] or proof['name'].casefold() not in plain(raw,proof['file']).casefold() or urllib.parse.unquote(url).rstrip('/') not in links or entity.get('claims',{}).get('P570'):return ['linked person bound source or entity changed or does not identify that source']
        except (OSError,KeyError,ValueError,TypeError):return ['linked person bound source or entity is unavailable']
    return []

def cmd_witness_person_bound(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);people=(data.get('witness') or {}).get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and p.get('role')==args.role and p.get('qid')==args.qid]
    if len(matches)!=1:die('record exactly one matching Wikidata participant first')
    person=matches[0];evidence_record=person.get('evidence') or {};note=args.bound_note
    if person.get('died') is not None or args.died_before>BOUND_BEFORE or not args.name or not trusted(args.source) or len(str(note or '').strip())<30:die('a source bound needs an unknown death, trusted source, name, ancient upper bound and detailed note')
    try:
        raw_entity=Path(witness_cache(path,args.language,evidence_record['file'])).read_bytes();entity=json.loads(raw_entity)['entities'][args.qid]
        if hashlib.sha256(raw_entity).hexdigest()!=evidence_record.get('sha256') or evidence_record.get('url')!=f'https://www.wikidata.org/wiki/Special:EntityData/{args.qid}.json' or entity.get('claims',{}).get('P570'):die('the exact entity evidence must be unchanged and contain no death claim')
    except (OSError,KeyError,ValueError,TypeError):die('the exact Wikidata entity evidence is missing or invalid')
    links=[urllib.parse.unquote(str(v.get('url'))).rstrip('/') for v in (entity.get('sitelinks') or {}).values()]
    if urllib.parse.unquote(args.source).rstrip('/') not in links:die('the reference must be a sitelink of this exact Wikidata entity')
    raw=get(args.source)
    if args.name.casefold() not in plain(raw,'person.html').casefold():die('the source does not contain the stated name')
    digest=hashlib.sha256(raw).hexdigest();filename=f'person-bound-{digest}.html';Path(witness_cache(path,args.language,filename)).write_bytes(raw)
    person.update({'died_before':args.died_before,'death_bound':{'year':args.died_before,'from':args.source,'note':note},'bound_reference':{'name':args.name,'url':args.source,'file':filename,'sha256':digest}})
    data['review']={};save(file,data);print('source-bound Wikidata participant recorded; independent review is required')

def cmd_witness_bound_review(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);review=data.get('review') or {};reader=review.get('reader');maker=(data.get('witness') or {}).get('maker')
    people=(data.get('witness') or {}).get('persons') or []
    matches=[p for p in people if isinstance(p,dict) and p.get('role')==args.role and (p.get('name') or {}).get('en')==args.name and p.get('died_before') is not None]
    if len(matches)!=1:die('choose one source-bounded person by exact name and role')
    if not reader or reader!=args.reader or reader==maker or not (review.get('places') or {}) or len(args.assessment.strip())<60:die('the independent witness reader must inspect the bound and explain it in at least 60 characters after the page review')
    if matches[0].get('qid'):
        issues=witness_linked_bound_problems(path,args.language,matches[0])
        if issues:die('; '.join(issues))
    review.setdefault('bounds',{})[page_number_digest(matches[0])]={'reader':reader,'assessment':args.assessment}
    data['review']=review;save(file,data);print('independent bound assessment recorded for this exact participant')

def cmd_witness_fetch(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if not lawful_url(args.url):die('source must be a lawful https URL')
    raw=open(args.file,'rb').read() if args.file else get(args.url)
    name=args.name
    digest=hashlib.sha256(raw).hexdigest();existing=next((item for item in data['sources'] if item.get('file')==name),None)
    if any(item.get('file')==name for item in data.get('images') or []):die('source filename is already an image; choose another name')
    if existing:
        if existing.get('url')!=args.url or existing.get('sha256')!=digest:die('source filename already records different evidence; choose another name')
        destination=Path(witness_cache(path,args.language,name))
        if destination.exists() and destination.read_bytes()!=raw:die('existing source bytes changed; preserve them for review')
        if not destination.exists():destination.write_bytes(raw)
        print(json.dumps(existing,ensure_ascii=False));return
    destination=Path(witness_cache(path,args.language,name))
    if destination.exists() and destination.read_bytes()!=raw:die('source filename already holds different bytes; choose another name')
    destination.write_bytes(raw)
    source={'file':name,'url':args.url,'sha256':hashlib.sha256(raw).hexdigest(),'fetched':today()}
    data['sources']=[item for item in data['sources'] if item['file']!=name]+[source]
    data['review']={}
    save(witness_file(path,args.language),data);print(json.dumps(source,ensure_ascii=False))

def image_ocr(image_path,model_dir,language):
    """Produce raw OCR from one recorded page with Tesseract's default segmentation."""
    import ctypes as c
    import ctypes.util
    try:t=c.CDLL(ctypes.util.find_library('tesseract') or 'libtesseract.so.5');lept=c.CDLL(ctypes.util.find_library('leptonica') or 'libleptonica.so.6')
    except OSError as error:die(f'Tesseract and Leptonica libraries are required: {error}')
    def bind(lib,name,args,result):
        fn=getattr(lib,name);fn.argtypes=args;fn.restype=result;return fn
    ptr=c.c_void_p;create=bind(t,'TessBaseAPICreate',[],ptr);init=bind(t,'TessBaseAPIInit3',[ptr,c.c_char_p,c.c_char_p],c.c_int)
    set_image=bind(t,'TessBaseAPISetImage2',[ptr,ptr],None);get_text=bind(t,'TessBaseAPIGetUTF8Text',[ptr],ptr)
    delete_text=bind(t,'TessDeleteText',[ptr],None);end=bind(t,'TessBaseAPIEnd',[ptr],None);delete=bind(t,'TessBaseAPIDelete',[ptr],None)
    version=bind(t,'TessVersion',[],c.c_char_p);pix_read=bind(lept,'pixRead',[c.c_char_p],ptr);pix_destroy=bind(lept,'pixDestroy',[c.POINTER(ptr)],None)
    lept_version=bind(lept,'getLeptonicaVersion',[],ptr);lept_free=bind(lept,'lept_free',[ptr],None)
    api=create()
    if not api:die('Tesseract could not create an OCR engine')
    try:
        if init(api,os.fsencode(model_dir),language.encode())!=0:die('Tesseract could not load the specified model')
        pix=ptr(pix_read(os.fsencode(image_path)))
        if not pix:die('Leptonica could not read the page')
        try:
            set_image(api,pix);text=get_text(api)
            if not text:die('Tesseract returned no OCR text')
            try:raw=c.string_at(text)
            finally:delete_text(text)
        finally:pix_destroy(c.byref(pix))
    finally:end(api);delete(api)
    lv=lept_version()
    try:lept_name=c.string_at(lv).decode()
    finally:lept_free(lv)
    if not raw.strip():die('Tesseract returned empty OCR text')
    return raw,version().decode(),lept_name

def cmd_witness_ocr(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if not re.fullmatch(r'[a-zA-Z0-9_]+',args.model):die('name one Tesseract model')
    image=next((i for i in data.get('images') or [] if i.get('file')==args.image and i.get('by')=='maker'),None)
    if not image or not lawful_url(image.get('url')):die('fetch the exact page as maker first')
    local=lambda name:witness_cache(path,args.language,name)
    try:
        page=Path(local(image['file'])).read_bytes();model=Path(args.model_dir,args.model+'.traineddata').read_bytes()
    except OSError:die('the page image or specified OCR model is unavailable')
    if hashlib.sha256(page).hexdigest()!=image.get('sha256') or not valid_page_image(page):die('the maker page changed or is not an image')
    raw,version,leptonica=image_ocr(local(image['file']),args.model_dir,args.model)
    runner=Path(__file__).read_bytes();runner_digest=hashlib.sha256(runner).hexdigest();runner_name='ocr-runner-'+runner_digest+'.py'
    digest=hashlib.sha256(raw).hexdigest();model_digest=hashlib.sha256(model).hexdigest();model_name='ocr-model-'+model_digest+'.traineddata'
    operation=hashlib.sha256((image['url']+'\0'+image['file']+'\0'+version+'\0'+leptonica+'\0'+args.model+'\0'+runner_digest).encode()).hexdigest()[:16]
    name='image-ocr-'+operation+'-'+image['sha256']+'-'+model_digest+'-'+digest+'.txt'
    for filename,content in ((model_name,model),(runner_name,runner),(name,raw)):
        target=Path(local(filename))
        if target.exists() and target.read_bytes()!=content:die('OCR evidence filename already holds different bytes')
        if not target.exists():target.write_bytes(content)
    record={'file':name,'url':image['url'],'sha256':digest,'derived_from':{'tool':'tesseract-image','file':image['file'],'sha256':image['sha256'],'version':version,'leptonica':leptonica,'language':args.model,'model_file':model_name,'model_sha256':model_digest,'runner_file':runner_name,'runner_sha256':runner_digest,'options':['default-page-segmentation','no-preprocessing']}}
    data['sources']=[s for s in data.get('sources') or [] if s.get('file')!=name]+[record];data['review']={};save(witness_file(path,args.language),data)
    print(f'{name}: raw local OCR from the recorded image; correct against the page and obtain an independent reading')

def cmd_witness_ocr_join(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);spec=load(args.file)
    if not isinstance(spec,dict) or set(spec)!={'segments','reason'} or not isinstance(spec['segments'],list) or len(spec['segments'])<2 or not isinstance(spec['reason'],str) or len(spec['reason'].strip())<40:die('give ordered OCR segments and explain page joins and omitted catchwords')
    local=lambda name:witness_cache(path,args.language,name);records=[];chunks=[];parents=[]
    for segment in spec['segments']:
        if not isinstance(segment,dict) or set(segment)!={'source','open','close'} or not all(isinstance(segment[k],str) and segment[k] for k in segment):die('each segment needs its source, exact opening and exact closing')
        source=next((s for s in data.get('sources') or [] if s.get('file')==segment['source'] and (s.get('derived_from') or {}).get('tool')=='tesseract-image'),None)
        if not source:die('each segment must come from a recorded page OCR')
        if derived_source_problems([source],local,images=data.get('images') or []):die('the OCR page or model evidence is invalid')
        try:raw=Path(local(source['file'])).read_bytes();text=raw.decode('utf-8')
        except (OSError,UnicodeDecodeError):die('the raw OCR must be available as UTF-8')
        if text.count(segment['open'])!=1:die('each opening must occur exactly once on its OCR page')
        start=text.index(segment['open']);end=text.find(segment['close'],start)
        if end<0 or text.find(segment['close'],end+1)>=0:die('the closing must occur exactly once after the opening on that page')
        end+=len(segment['close']);records.append({'file':source['file'],'sha256':source['sha256'],'start':start,'end':end});chunks.append(text[start:end].strip());parents.append(source)
    if len({(p['derived_from']['file'],p['derived_from']['sha256']) for p in parents})!=len(parents):die('each joined segment must name a different page image')
    raw=' '.join(chunks).encode('utf-8');digest=hashlib.sha256(raw).hexdigest();proof={'tool':'ocr-page-join','segments':records,'reason':spec['reason']}
    name='joined-ocr-'+page_number_digest(proof)+'-'+digest+'.txt';target=Path(local(name))
    if target.exists() and target.read_bytes()!=raw:die('joined OCR filename already holds different bytes')
    if not target.exists():target.write_bytes(raw)
    source={'file':name,'url':parents[0]['url'],'sha256':digest,'derived_from':proof};data['sources']=[s for s in data.get('sources') or [] if s.get('file')!=name]+[source];data['review']={};save(witness_file(path,args.language),data)
    print(f'{name}: ordered raw page segments joined; record every page and obtain independent review')

def cmd_witness_ocr_join_review(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc) or {};review=data.get('review') or {};reader=review.get('reader')
    source=next((s for s in data.get('sources') or [] if s.get('file')==place.get('source') and (s.get('derived_from') or {}).get('tool')=='ocr-page-join'),None)
    if not source or not reader or reader!=args.reader or reader==(data.get('witness') or {}).get('maker') or not (review.get('places') or {}).get(args.loc) or len(args.assessment.strip())<60:die('the independent page reader must assess the join and omitted catchwords after reading all pages')
    if derived_source_problems(data.get('sources') or [],lambda name:witness_cache(path,args.language,name),images=data.get('images') or []):die('the source evidence must be intact before reviewing the join')
    review.setdefault('ocr_joins',{})[args.loc]={'reader':reader,'sha256':page_number_digest({'source':source,'place':place}),'assessment':args.assessment};data['review']=review;save(witness_file(path,args.language),data)
    print('independent OCR page join and omission assessment recorded')

def cmd_witness_restore_evidence(args):
    """Restore only the exact missing bytes, without altering the signed witness."""
    path=packet(args.packet);data=witness_data(path,args.language)
    matches=[item for item in [*(data.get('sources') or []),*(data.get('images') or [])] if item.get('file')==args.name]
    if len(matches)!=1:die('choose exactly one recorded source or image')
    entry=matches[0];destination=Path(witness_cache(path,args.language,args.name))
    if destination.exists():die('evidence bytes already exist; preserve and inspect them')
    if not lawful_url(entry.get('url')) or not re.fullmatch(r'[a-f0-9]{64}',str(entry.get('sha256',''))):die('recorded evidence needs its lawful source and SHA-256')
    if not args.file and (entry.get('capture') or entry.get('derived_from')):die('derived images or text require the exact saved bytes, not a fetch of their parent URL')
    raw=Path(args.file).read_bytes() if args.file else get(entry['url'])
    if hashlib.sha256(raw).hexdigest()!=entry['sha256']:die('restored bytes do not match the recorded evidence')
    destination.write_bytes(raw);print('the exact missing evidence bytes were restored; the witness and its review were not changed')

def cmd_witness_source_remove(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);sources=data.get('sources') or []
    matches=[source for source in sources if source.get('file')==args.name]
    if len(matches)!=1:die('exactly one source with that filename must be present')
    if len(args.reason.strip())<30:die('explain the source removal in a full sentence')
    def references(value):
        if isinstance(value,dict):return any(k==args.name or references(v) for k,v in value.items())
        if isinstance(value,list):return any(references(v) for v in value)
        return value==args.name
    remaining=[source for source in sources if source is not matches[0]]
    if references({key:value for key,value in data.items() if key not in ('sources','review','source_corrections')}) or references(remaining):die('this source is still referenced by the witness or another source; retain or replace its uses first')
    data['sources']=[source for source in sources if source is not matches[0]]
    data.setdefault('source_corrections',[]).append({'removed':matches[0],'reason':args.reason,'date':today()})
    data['review']={};save(file,data)
    print('unused source removed with history; independent review must be renewed')

def cmd_witness_image(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if not lawful_url(args.url):die('image must be from a lawful https URL')
    if args.by=='second' and any(item.get('file')==args.name for item in [*(data.get('sources') or []),*(item for item in data.get('images',[]) if item.get('by')=='maker')]):die('the second reader must use a separate filename; preserve maker source and image bytes')
    raw=open(args.file,'rb').read() if args.file else get(args.url)
    if not valid_page_image(raw):die('the page source did not yield a readable image')
    name=args.name
    if any(item.get('file')==name for item in data.get('sources') or []):die('image filename is already a source; choose another name')
    existing=next((item for item in data.get('images') or [] if item.get('file')==name),None);digest=hashlib.sha256(raw).hexdigest()
    if existing:
        if existing.get('by')!=args.by or existing.get('url')!=args.url or existing.get('sha256')!=digest:die('image filename already records different evidence; choose another name')
        destination=Path(witness_cache(path,args.language,name))
        if destination.exists() and destination.read_bytes()!=raw:die('existing image bytes changed; preserve them for review')
        if not destination.exists():destination.write_bytes(raw)
        print(json.dumps(existing,ensure_ascii=False));return
    destination=Path(witness_cache(path,args.language,name))
    if destination.exists() and destination.read_bytes()!=raw:die('image filename already holds different bytes; choose another name')
    destination.write_bytes(raw)
    image={'file':name,'url':args.url,'sha256':hashlib.sha256(raw).hexdigest(),'by':args.by}
    data['images']=[item for item in data['images'] if not (item['file']==name and item['by']==args.by)]+[image]
    if args.by=='maker':data['review']={}
    save(witness_file(path,args.language),data);print(json.dumps(image,ensure_ascii=False))

def browser_capture_problems(path,lang,data,image,offline=False):
    proof=image.get('capture') or {};sources=data.get('sources') or [];witness=data.get('witness') or {}
    if not isinstance(proof,dict):return ['invalid browser capture evidence']
    source=next((item for item in sources if item.get('file')==proof.get('source_file') and item.get('sha256')==proof.get('source_sha256') and item.get('url')==image.get('url')),None)
    try:timestamp=datetime.datetime.fromisoformat(proof.get('captured_at','').replace('Z','+00:00'));dated=timestamp.tzinfo is not None
    except (ValueError,TypeError):dated=False
    by=image.get('by');reader=proof.get('reader')
    if not source or proof.get('method')!='codex-iab-screenshot' or not dated or by not in ('maker','second') or not reader or (by=='maker' and reader!=witness.get('maker')) or (by=='second' and reader==witness.get('maker')):return ['browser capture needs the exact fetched page, capture time, method and reader identity']
    if by=='second' and (proof.get('independent_fetch') or {})!={'url':source.get('url'),'sha256':source.get('sha256')}:return ['second browser capture requires an independent fetch of the same page bytes']
    if not offline:
        try:
            local=(lambda name:witness_cache(path,lang,name)) if lang else (lambda name:cache(path,name))
            html=Path(local(source['file'])).read_bytes();raw=Path(local(image['file'])).read_bytes()
            if hashlib.sha256(html).hexdigest()!=source.get('sha256') or hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):return ['browser capture or source page changed']
        except (OSError,KeyError):return ['browser capture or source page is missing']
    return []

def cmd_witness_browser_capture(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);source=next((item for item in data.get('sources') or [] if item.get('file')==args.source and not item.get('derived_from')),None)
    if not source or not lawful_url(source.get('url')):die('fetch the exact lawful web page first')
    html=Path(witness_cache(path,args.language,source['file'])).read_bytes()
    if hashlib.sha256(html).hexdigest()!=source.get('sha256') or not re.search(rb'<(?:!doctype\s+html|html|body)\b',html[:8192],re.I):die('the exact HTML source must be unchanged and identify an HTML document')
    raw=Path(args.file).read_bytes()
    if not valid_page_image(raw):die('the browser capture is not a readable image')
    digest=hashlib.sha256(raw).hexdigest();key=hashlib.sha256((source['url']+'\0'+source['file']+'\0'+args.reader+'\0'+args.captured_at).encode()).hexdigest()[:16]
    name=f'browser-page-{key}-{digest}-{args.by}.img'
    proof={'method':'codex-iab-screenshot','source_file':source['file'],'source_sha256':source['sha256'],'captured_at':args.captured_at,'reader':args.reader}
    if args.by=='second':
        if not any(image.get('by')=='maker' and (image.get('capture') or {}).get('source_file')==source['file'] and (image.get('capture') or {}).get('source_sha256')==source['sha256'] for image in data.get('images') or []):die('record the maker capture of this page first')
        independent=get(source['url'])
        if hashlib.sha256(independent).hexdigest()!=source['sha256']:die('the independently fetched page differs from the maker source')
        proof['independent_fetch']={'url':source['url'],'sha256':source['sha256']}
    image={'file':name,'url':source['url'],'sha256':digest,'by':args.by,'capture':proof}
    Path(witness_cache(path,args.language,name)).write_bytes(raw)
    issues=browser_capture_problems(path,args.language,data,image)
    if issues:die('; '.join(issues))
    data['images']=[item for item in data.get('images',[]) if item.get('file')!=name]+[image]
    if args.by=='maker':data['review']={}
    save(file,data);print(f'{name}: browser capture recorded as a human visual observation, not as downloaded page bytes')

def second_witness_image(data,page,reader):
    images=data.get('images') or []
    maker=next((item for item in images if item.get('by','maker')=='maker' and item.get('url')==page.get('url') and item.get('sha256')==page.get('image_sha256')),None)
    if not maker:return None
    capture=maker.get('capture')
    if capture and not isinstance(capture,dict):return None
    if capture:
        return next((item for item in images if item.get('by')=='second' and item.get('url')==page.get('url') and isinstance(item.get('capture'),dict) and item['capture'].get('reader')==reader and item['capture'].get('source_file')==capture.get('source_file') and item['capture'].get('source_sha256')==capture.get('source_sha256')),None)
    exact=next((item for item in images if item.get('by')=='second' and item.get('url')==page.get('url') and item.get('sha256')==page.get('image_sha256')),None)
    if exact:return exact
    for proof in ((data.get('review') or {}).get('page_identities') or []):
        if not isinstance(proof,dict) or proof.get('reader')!=reader or reader==(data.get('witness') or {}).get('maker') or not lawful_url(proof.get('reader_url')) or len(str(proof.get('assessment','')).strip())<80 or len(str(proof.get('method','')).strip())<20:continue
        second=next((i for i in images if i.get('by')=='second' and i.get('file')==proof.get('second_file')),None)
        if second and lawful_url(second.get('url')) and proof.get('sha256')==page_number_digest({'page':page,'maker':maker,'second':second,'edition':(data.get('witness') or {}).get('edition')}):return second
    return None

def cmd_witness_page_identity(args):
    """A second reader explicitly compares two separately retrieved renderings."""
    path=packet(args.packet);data=editable_witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc)
    if not isinstance(place,dict):die('no such place in this witness')
    pages=[p for p in witness_pages(place) if p.get('page')==args.page]
    if len(pages)!=1:die('select exactly one recorded printed page')
    page=pages[0];images=data.get('images') or []
    maker=next((i for i in images if i.get('by')=='maker' and i.get('url')==page.get('url') and i.get('sha256')==page.get('image_sha256')),None)
    second=next((i for i in images if i.get('by')=='second' and i.get('file')==args.image),None)
    if not maker or not second or not lawful_url(second.get('url')) or not args.reader or args.reader==(data.get('witness') or {}).get('maker') or not lawful_url(args.reader_url) or len(args.method.strip())<20 or len(args.assessment.strip())<80:die('a distinct reader, both images, lawful reader URL, retrieval method and specific page identity assessment are required')
    if maker.get('capture') or second.get('capture'):die('browser captures use their own source-bound route')
    for image in (maker,second):
        try:raw=Path(witness_cache(path,args.language,image['file'])).read_bytes()
        except (OSError,KeyError):die('both reader images must be present')
        if hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):die('a reader image changed or is invalid')
    review=data.setdefault('review',{})
    if review.get('reader') and review['reader']!=args.reader:die('a single second reader reviews this witness')
    review['reader']=args.reader;review.setdefault('places',{}).pop(args.loc,None)
    proof={'reader':args.reader,'reader_url':args.reader_url,'method':args.method,'assessment':args.assessment,'second_file':second['file'],'sha256':page_number_digest({'page':page,'maker':maker,'second':second,'edition':(data.get('witness') or {}).get('edition')})}
    review.setdefault('page_identities',[]).append(proof);save(witness_file(path,args.language),data)
    print('independent page identity assessment recorded; the reader must still review the text and rights')

def original_browser_problems(path,ev,reading,places,second=False,offline=False):
    problems=[];images=ev.get('images') or [];maker=(reading.get('reader') or {}).get('id');review=part(path,'review.json') or {};reader=(review.get('reader') or {}).get('id')
    data={'sources':ev.get('sources') or [],'witness':{'maker':maker}}
    for loc,entry in (ev.get('places') or {}).items():
        if entry.get('scan')=='web' and loc in (places.get('places') or {}) and not any(isinstance(i,dict) and i.get('item')=='web' and i.get('loc')==loc and i.get('by')=='maker' and i.get('capture') for i in images):problems.append(f'{loc}: web place needs its recorded maker capture')
    for image in images:
        if not isinstance(image,dict) or image.get('item')!='web':continue
        if not image.get('capture'):problems.append('web image is missing its capture provenance');continue
        loc=image.get('loc');proof=image.get('capture') or {};place=(places.get('places') or {}).get(loc)
        if not place:continue  # a withdrawn place stays in evidence
        issues=browser_capture_problems(path,None,data,image,offline=offline)
        if issues:problems.extend(f'{loc}: {issue}' for issue in issues);continue
        source=next((s for s in data['sources'] if s.get('file')==proof.get('source_file')),None);entry=(ev.get('places') or {}).get(loc) or {};original=original_of(places)
        address=(place.get('addresses') or {}).get(original) or {};captures=[i for i in images if isinstance(i,dict) and i.get('item')=='web' and i.get('loc')==loc and i.get('n')==image.get('n')]
        if not source or source.get('role')!='transcription' or entry.get('transcription')!=source.get('file') or address.get('kind') not in ('section','chapter','paragraph') or not address.get('value') or address.get('url')!=source.get('url') or proof.get('address')!=address or (place.get('links') or {}).get(original)!=source.get('url') or entry.get('scan')!='web' or image.get('n') not in entry.get('images',[]):
            problems.append(f'{loc}: web capture needs the exact source and structural address of this place');continue
        if not offline:
            try:
                raw=Path(cache(path,source['file'])).read_bytes();text=Path(cache(path,source['text'])).read_text(encoding='utf-8')
                if plain(raw,source['file'])!=text:problems.append(f'{loc}: web transcription differs from its fetched HTML')
            except (OSError,KeyError,UnicodeError):problems.append(f'{loc}: web transcription is unavailable')
        if second and image.get('by')=='maker':
            matched=next((i for i in captures if i.get('by')=='second' and isinstance(i.get('capture'),dict) and i['capture'].get('reader')==reader and i['capture'].get('source_file')==proof.get('source_file') and i['capture'].get('source_sha256')==proof.get('source_sha256') and i.get('url')==image.get('url')),None)
            looked=any(item.get('loc')==loc and item.get('n')==image.get('n') and item.get('reads') in ('as the packet','differs') for item in review.get('images') or [])
            if not matched or not looked or not reader or reader==maker:problems.append(f'{loc}: the independent reader must capture and assess this same web section')
    return problems

def cmd_original_browser_capture(args):
    path=packet(args.packet)
    if part(path,'accepted.json') is not None:die('an accepted packet cannot be changed')
    ev=evidence(path);reading=part(path,'reading.json') or {};places=part(path,'places.json') or {};maker=(reading.get('reader') or {}).get('id');place=(places.get('places') or {}).get(args.loc);entry=(ev.get('places') or {}).get(args.loc) or {}
    source=next((s for s in ev.get('sources') or [] if s.get('file')==args.source and s.get('role')=='transcription' and not s.get('derived_from')),None)
    if not place or not source or entry.get('transcription')!=args.source or not maker or args.n<1 or args.address_kind not in ('section','chapter','paragraph') or not args.address_value.strip():die('cut this original place from the exact fetched HTML and give a structural address and capture index')
    html=Path(cache(path,source['file'])).read_bytes()
    if hashlib.sha256(html).hexdigest()!=source.get('sha256') or not re.search(rb'<(?:!doctype\s+html|html|body)\b',html[:8192],re.I):die('the unchanged transcription source must be HTML')
    raw=Path(args.file).read_bytes()
    if not valid_page_image(raw):die('the browser capture is not a readable image')
    proof={'method':'codex-iab-screenshot','source_file':source['file'],'source_sha256':source['sha256'],'captured_at':args.captured_at,'reader':args.reader,'address':{'kind':args.address_kind,'value':args.address_value,'url':source['url']}}
    if args.by=='second':
        if not any(i.get('item')=='web' and i.get('loc')==args.loc and i.get('n')==args.n and i.get('by')=='maker' and (i.get('capture') or {}).get('source_file')==source['file'] for i in ev.get('images') or []):die('record the maker capture for this place first')
        if hashlib.sha256(get(source['url'])).hexdigest()!=source['sha256']:die('the independently fetched page differs from the maker source')
        proof['independent_fetch']={'url':source['url'],'sha256':source['sha256']}
    digest=hashlib.sha256(raw).hexdigest();key=hashlib.sha256((source['url']+'\0'+args.loc+'\0'+args.reader+'\0'+args.captured_at).encode()).hexdigest()[:16];name=f'web-{key}-{digest}-{args.by}.img'
    image={'item':'web','n':args.n,'loc':args.loc,'file':name,'url':source['url'],'sha256':digest,'by':args.by,'capture':proof};Path(cache(path,name)).write_bytes(raw)
    issues=browser_capture_problems(path,None,{'sources':ev['sources'],'witness':{'maker':maker}},image)
    if issues:die('; '.join(issues))
    if args.by=='maker':
        scan={'item':'web','ocr':False,'image_offset':0};ev['scans']=scans(ev);ev['scans']['web']=scan
        if not ev.get('scan'):ev['scan']=scan
        previous=dict(entry);same=scan_of(ev,entry).get('item')=='web'
        entry.update({'method':'image','scan':'web','images':sorted(set(entry.get('images',[]) if same else [])|{args.n})});ev.setdefault('places',{})[args.loc]=entry
        original=original_of(places);place.setdefault('links',{})[original]=source['url'];place.setdefault('addresses',{})[original]={'kind':args.address_kind,'value':args.address_value,'url':source['url']}
        prior=part(path,'review.json')
        if prior is not None:
            if previous!=entry:ev.setdefault('page_rebindings',[]).append({'loc':args.loc,'from':previous,'review':dict(prior),'date':today()})
            prior['images']=[item for item in prior.get('images',[]) if item.get('loc')!=args.loc];prior['accept']=False;save(os.path.join(path,'review.json'),prior)
    elif (place.get('addresses') or {}).get(original_of(places))!={'kind':args.address_kind,'value':args.address_value,'url':source['url']}:die('the second capture must name the same structural address')
    ev['images']=[i for i in ev.get('images',[]) if i.get('file')!=name]+[image];save(os.path.join(path,'evidence.json'),ev);save(os.path.join(path,'places.json'),places)
    print('browser capture recorded; the structural address is not a printed page number')

def cmd_witness_cut(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if args.loc in (data.get('places') or {}):die('this place already exists; correct it or withdraw it with history before recutting')
    original={**((world()['fragments'].get('places') or {}).get(work_of(path),{})),**((part(path,'places.json') or {}).get('places') or {})}
    if args.loc not in original:die('the original place is not in the bonsai')
    source=next((item for item in data['sources'] if item['file']==args.source),None)
    if not source:die('fetch the source first')
    raw=Path(witness_cache(path,args.language,args.source)).read_bytes()
    if hashlib.sha256(raw).hexdigest()!=source['sha256']:die('source bytes have changed')
    if raw.startswith(b'%PDF-'):die('derive PDF text with kit.py pdf-text before cutting a witness')
    text=plain(raw,args.source)
    pattern=lambda phrase:re.compile(r'\s+'.join(re.escape(word) for word in phrase.split()))
    opens=list(pattern(args.open).finditer(text))
    if len(opens)!=1:die(f'opening must occur once; found {len(opens)}')
    start=opens[0].start();close=pattern(args.close).search(text,start)
    if not close:die('closing is absent after the opening')
    run=squeeze(text[start:close.end()])
    problem=in_transcription(run,text)
    if problem:die(problem)
    if args.language=='ru':
        modern=modern_russian(run,old_source=old_russian_source(text))
        if modern!=run:data['witness']['normalization']='Russian spelling modernized from the source by the 1918 reform rules in access/web/src/public-research/orthography.mjs, with ѵ rendered as и; wording unchanged.'
        run=modern
    image=next((item for item in data['images'] if item['file']==args.image and item['by']=='maker'),None)
    if not image:die('fetch the page image as maker first')
    if (source.get('derived_from') or {}).get('tool')=='tesseract-image' and (source['derived_from'].get('sha256')!=image.get('sha256') or source.get('url')!=image.get('url')):die('this OCR came from a different page image')
    if (source.get('derived_from') or {}).get('tool')=='ocr-page-join' and source.get('url')!=image.get('url'):die('the first page of the joined OCR must be the primary image')
    address={'kind':args.address_kind,'value':args.address_value,'url':image['url']} if args.address_kind or args.address_value else None
    if address and not valid_address(address):die('structured address needs kind, value and the source URL')
    if not args.page and not address:die('give a printed page or an exact structured source address')
    page=args.page or f"{address['kind']} {address['value']}"
    data['places'][args.loc]={'text':run,'source':args.source,'page':page,'url':image['url'],'image_sha256':image['sha256'],'alignment':args.alignment,**({'address':address} if address else {}),**({'lines':args.lines} if args.lines else {}),**({'justification':args.justification} if args.justification else {})}
    (data.get('missing_places') or {}).pop(args.loc,None)
    data['review']={} # the old second reading no longer describes this witness
    save(witness_file(path,args.language),data);print(run)

def cmd_witness_address_correct(args):
    """Correct a single page label without recutting its text or losing its history."""
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    place=(data.get('places') or {}).get(args.loc)
    if not isinstance(place,dict):die('no such place in this witness')
    if not re.fullmatch(r'\s*(?:p(?:age)?\.?\s*)?\d+[a-zA-Z]?\s*',args.page):die('give one printed page, separate from the line range')
    if args.line_start<1 or args.line_end<args.line_start:die('line range must have positive ascending bounds')
    section=getattr(args,'section',None)
    if section is not None and (not section.strip() or len(section.strip())>100):die('section must be a short explicit locator')
    if len(args.reason.strip())<40:die('explain the address correction in at least 40 characters')
    image=next((i for i in data.get('images') or [] if i.get('by')=='maker' and i.get('url')==place.get('url') and i.get('sha256')==place.get('image_sha256')),None)
    if not image:die('the original maker page image is required')
    try:raw=Path(witness_cache(path,args.language,image['file'])).read_bytes()
    except OSError:die('the maker page image is missing')
    if hashlib.sha256(raw).hexdigest()!=image['sha256'] or not valid_page_image(raw):die('the maker page image changed or is invalid')
    if place.get('extra_pages'):die('this route only corrects a single-page place')
    old=json.loads(json.dumps(place));previous_review=data.get('review') or {}
    place['page']=args.page.strip();place['line_locator']={'start':args.line_start,'end':args.line_end,**({'section':section.strip()} if section else {})}
    for correction in place.get('corrections') or []:
        if all(correction.get(k)==old.get(k) for k in ('page','url','image_sha256')):correction['page']=place['page']
    data.setdefault('address_corrections',[]).append({'loc':args.loc,'predecessor':old,'predecessor_sha256':page_number_digest(old),'page':place['page'],'line_locator':place['line_locator'],'reason':args.reason,'review':previous_review,'date':today()})
    data['review']={};save(witness_file(path,args.language),data)
    print('address corrected with predecessor and image retained; independent review must be renewed')

def cmd_witness_transcribe_page(args):
    """Record a human reading of an image, without claiming a fetched transcription."""
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if args.loc in (data.get('places') or {}):die('this place already exists; correct it or withdraw it with history before retranscribing')
    original={**((world()['fragments'].get('places') or {}).get(work_of(path),{})),**((part(path,'places.json') or {}).get('places') or {})}
    if args.loc not in original:die('the original place is not in the bonsai')
    image=next((item for item in data['images'] if item['file']==args.image and item['by']=='maker'),None)
    if not image:die('fetch the page image as maker first')
    try:raw=Path(witness_cache(path,args.language,image['file'])).read_bytes()
    except OSError:die('the maker page image is missing')
    if hashlib.sha256(raw).hexdigest()!=image['sha256'] or not valid_page_image(raw):die('the maker page image changed or is invalid')
    address={'kind':args.address_kind,'value':args.address_value,'url':image['url']} if args.address_kind or args.address_value else None
    if address and not valid_address(address):die('structured address needs kind, value and the source URL')
    if not args.page and not address:die('give a printed page or an exact structured source address')
    if len(args.reason.strip())<40:die('explain why this page needs a human transcription')
    run=squeeze(args.text);page_text=run
    if not run:die('transcribe the page text')
    if args.language=='ru':
        modern=modern_russian(run,old_source=old_russian_source(run))
        if modern!=run:data['witness']['normalization']='Russian spelling modernized from the page by the 1918 reform rules in access/web/src/public-research/orthography.mjs, with ѵ rendered as и; wording unchanged.'
        run=modern
    page=args.page or f"{address['kind']} {address['value']}"
    data['places'][args.loc]={'text':run,'transcription':{'kind':'human-page','reason':args.reason,'original_text':page_text},'page':page,'url':image['url'],'image_sha256':image['sha256'],'alignment':args.alignment,**({'address':address} if address else {}),**({'lines':args.lines} if args.lines else {}),**({'justification':args.justification} if args.justification else {})}
    (data.get('missing_places') or {}).pop(args.loc,None)
    data['review']={};save(witness_file(path,args.language),data);print(run)

def witness_pages(place):
    return [{'page':place.get('page'),'url':place.get('url'),'image_sha256':place.get('image_sha256')},*(place.get('extra_pages') or [])]

def new_translation_problems(path,lang,data,loc,second=False,offline=False):
    problems=[];witness=data.get('witness') or {};place=(data.get('places') or {}).get(loc) or {};proof=place.get('transcription') or {};creation=witness.get('created_translation') or {};world_=world()['fragments'];work=(world_.get('works') or {}).get(work_of(path)) or {};original=(world_.get('places') or {}).get(work_of(path),{}).get(loc) or {};original_lang=work.get('original');original_text=(original.get('texts') or {}).get(original_lang);source=(work.get('witnesses') or {}).get((original.get('witness') or {}).get(original_lang,original_lang)) or {}
    if not isinstance(proof,dict) or not isinstance(creation,dict):return ['invalid newly created translation record']
    people=witness.get('persons') or [];creator=creation.get('creator');licence=witness.get('licence_evidence') or {};licence_source=licence.get('source') or {}
    translators=[p for p in people if isinstance(p,dict) and p.get('role')=='translator' and isinstance(p.get('name'),dict) and creator in p['name'].values()]
    if len(translators)!=1 or not creator or creation.get('kind')!='new-translation' or not isinstance(witness.get('edition'),str) or 'new translation' not in witness['edition'].casefold() or creator not in str(witness.get('attribution') or ''):problems.append('a new translation needs its named sourced translator and explicitly new edition and attribution')
    translation_rights=witness.get('rights') or {};translation_rights=translation_rights if isinstance(translation_rights,dict) else {}
    if not licence_source.get('url') or witness.get('url')!=licence_source.get('url') or any(not isinstance(translation_rights.get(country),dict) or translation_rights[country].get('basis')!='licensed' for country in JURISDICTIONS):problems.append('the new translation needs its own independently reviewed licence declaration in every jurisdiction')
    if not offline:
        try:
            raw=Path(witness_cache(path,lang,licence_source['file'])).read_bytes();content=(pdf_layer(raw) if raw.startswith(b'%PDF-') else plain(raw,licence_source['file'])).casefold()
            if hashlib.sha256(raw).hexdigest()!=licence_source.get('sha256') or not creator or str(creator).casefold() not in content or str(witness.get('edition') or '').casefold() not in content:problems.append('the exact fetched creator declaration must name this translator and new edition')
        except (OSError,KeyError,TypeError):problems.append('the new translation licence declaration is unavailable')
    rights=source.get('rights') or {}
    if not isinstance(rights,dict) or any(not isinstance(rights.get(country),dict) or rights[country].get('basis') not in ('public-domain','public-domain+licence','licensed') for country in JURISDICTIONS):problems.append('the exact original edition needs public-domain or licensed reuse in every jurisdiction')
    if isinstance(rights,dict) and any(isinstance(rights.get(country),dict) and rights[country].get('basis') in ('licensed','public-domain+licence') for country in JURISDICTIONS):
        license_url=((source.get('licence_evidence') or {}).get('licence_url') or '').lower()
        if not license_url or '/by-nd/' in license_url or '/by-nc-nd/' in license_url:problems.append('the original licence must permit adaptation')
    source_url=(original.get('links') or {}).get(original_lang) or original.get('url');image=next((i for i in data.get('images') or [] if i.get('by')=='maker' and i.get('url')==place.get('url') and i.get('sha256')==place.get('image_sha256')),None)
    if not isinstance(original_text,str) or not original_text or not lawful_url(source_url) or proof.get('original_url')!=source_url or proof.get('original_language')!=original_lang or proof.get('original_sha256')!=hashlib.sha256(original_text.encode()).hexdigest() or proof.get('original_witness_sha256')!=page_number_digest(source) or not image or len(str(proof.get('source_assessment','')).strip())<60 or len(str(proof.get('image_assessment','')).strip())<40:problems.append('the new translation needs the exact original words, edition, source page and reuse assessment')
    if second:
        review=data.get('review') or {};reader=review.get('reader');check=(review.get('new_translations') or {}).get(loc) or {}
        if not reader or reader==witness.get('maker') or check.get('reader')!=reader or check.get('sha256')!=page_number_digest(place) or len(str(check.get('assessment','')).strip())<60:problems.append('the new translation needs independent linguistic and source-reuse assessment')
    return problems

def cmd_witness_new_translation(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);witness=data.get('witness') or {};world_=world()['fragments'];work=(world_.get('works') or {}).get(work_of(path)) or {};original=(world_.get('places') or {}).get(work_of(path),{}).get(args.loc) or {};original_lang=work.get('original');original_text=(original.get('texts') or {}).get(original_lang);source=(work.get('witnesses') or {}).get((original.get('witness') or {}).get(original_lang,original_lang)) or {};image=next((i for i in data.get('images') or [] if i.get('file')==args.image and i.get('by')=='maker'),None)
    if args.loc in (data.get('places') or {}):die('this witness already has the place; retain its history')
    if args.language==original_lang or not isinstance(original_text,str) or not original_text or not image or not args.page.strip() or not args.text.strip() or len(args.alignment.strip())<20:die('give another language, an existing original, its maker page, the new translation and its alignment')
    if args.language=='ru' and modern_russian(args.text)!=args.text:die('new Russian translation must use modern spelling')
    previous=witness.get('created_translation')
    creation={'kind':'new-translation','creator':args.creator}
    if previous and previous!=creation:die('the witness already names a different translation creator')
    witness['created_translation']=creation
    proof={'kind':'new-translation','translation_text':args.text,'original_url':(original.get('links') or {}).get(original_lang) or original.get('url'),'original_language':original_lang,'original_sha256':hashlib.sha256(original_text.encode()).hexdigest(),'original_witness_sha256':page_number_digest(source),'source_assessment':args.source_assessment,'image_assessment':args.image_assessment}
    place={'text':args.text,'transcription':proof,'page':args.page,'url':image['url'],'image_sha256':image['sha256'],'alignment':args.alignment}
    data.setdefault('places',{})[args.loc]=place;issues=new_translation_problems(path,args.language,data,args.loc)
    if issues:die('; '.join(issues))
    (data.get('missing_places') or {}).pop(args.loc,None);data['review']={};save(witness_file(path,args.language),data);print('new translation recorded as locally authored text; independent linguistic, licence and source review required')

def cmd_witness_new_translation_review(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc) or {};review=data.get('review') or {};reader=review.get('reader')
    if not reader or reader!=args.reader or reader==(data.get('witness') or {}).get('maker') or not (review.get('places') or {}).get(args.loc) or len(args.assessment.strip())<60:die('the independent page reader must assess translation, source and adaptation rights')
    issues=new_translation_problems(path,args.language,data,args.loc)
    if issues:die('; '.join(issues))
    review.setdefault('new_translations',{})[args.loc]={'reader':reader,'sha256':page_number_digest(place),'assessment':args.assessment};data['review']=review;save(witness_file(path,args.language),data);print('independent new-translation assessment recorded')

def cmd_witness_add_page(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    place=(data.get('places') or {}).get(args.loc) or die('no such place in this witness')
    image=next((item for item in data.get('images',[]) if item.get('file')==args.image and item.get('by')=='maker'),None)
    if not image:die('fetch the additional page image as maker first')
    try:raw=Path(witness_cache(path,args.language,image['file'])).read_bytes()
    except (OSError,KeyError):die('the additional page image is missing')
    if hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):die('the additional page image changed or is invalid')
    if not args.page.strip():die('give the printed page of the additional image')
    if not args.span.strip() or args.span not in place.get('text',''):die('the words on the added page must occur in the place')
    first=args.first_span or place.get('page_span')
    if not first or not place.get('text','').startswith(first):die('give the opening words on the first page with --first-span')
    if args.page==place.get('page'):die('the primary printed page is already recorded')
    if image['sha256']==place.get('image_sha256') and image['url']!=place.get('url'):die('a shared spread must retain the same source URL')
    page={'page':args.page,'url':image['url'],'image_sha256':image['sha256'],'span':args.span}
    place['page_span']=first;extra=place.setdefault('extra_pages',[])
    index=next((i for i,item in enumerate(extra) if item.get('page')==args.page),None)
    if index is None:extra.append(page)
    else:extra[index]=page
    data['review']={};save(witness_file(path,args.language),data)
    print(f'{args.loc}: additional page recorded; every page requires independent review')

def cmd_witness_correct(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    place=(data.get('places') or {}).get(args.loc) or die('no such place in this witness')
    old=getattr(args,'from')
    if not old or not args.to or old==args.to:die('the correction must change a nonempty run into a nonempty run; include context for a deletion')
    if place.get('text','').count(old)!=1:die('the original run must occur exactly once in the place')
    if len(args.reason.strip())<30:die('explain the page reading in a full sentence')
    pages=witness_pages(place)
    candidates=pages if not getattr(args,'image',None) else [page for page in pages if any(item.get('file')==args.image and item.get('by')=='maker' and item.get('sha256')==page.get('image_sha256') and item.get('url')==page.get('url') for item in data.get('images',[]))]
    if getattr(args,'page',None):candidates=[page for page in candidates if page.get('page')==args.page]
    elif not getattr(args,'image',None):candidates=candidates[:1]
    if len(candidates)!=1:die('select exactly one bound page; for a shared spread give --page')
    selected=candidates[0]
    image=next((item for item in data.get('images',[]) if item.get('by')=='maker' and item.get('sha256')==selected.get('image_sha256') and item.get('url')==selected.get('url')),None)
    if not image:die('the maker page image for this place is required')
    try:raw=Path(witness_cache(path,args.language,image['file'])).read_bytes()
    except (OSError,KeyError):die('the maker page image is missing')
    if hashlib.sha256(raw).hexdigest()!=selected['image_sha256']:die('the maker page image has changed')
    if place.get('extra_pages'):
        span=place.get('page_span') if selected is pages[0] else selected.get('span')
        if not isinstance(span,str) or span.count(old)!=1:die('the correction must lie wholly within the selected page span')
        updated=span.replace(old,args.to,1)
        if selected is pages[0]:place['page_span']=updated
        else:selected['span']=updated
    offset=place['text'].index(old)
    place['text']=place['text'][:offset]+args.to+place['text'][offset+len(old):]
    place.setdefault('corrections',[]).append({'from':old,'to':args.to,'offset':offset,'reason':args.reason,'url':selected['url'],'page':selected['page'],'image_sha256':selected['image_sha256']})
    data['review']={};save(witness_file(path,args.language),data)
    print(f'{args.loc}: correction recorded; compare on the page and obtain a new second reading')

def witness_emphasis_problems(path,lang,data,loc,second=False,offline=False):
    place=(data.get('places') or {}).get(loc) or {};marks=place.get('emphasis');text=place.get('text') or '';problems=[]
    if not isinstance(text,str) or not isinstance(marks,list) or not marks:return ['emphasis must list the exact printed words and page']
    if lang=='ru' and modern_russian(text)!=text:problems.append('normalize Russian text before binding displayed emphasis')
    used=[]
    for mark in marks:
        if not isinstance(mark,dict):problems.append('invalid emphasis record');continue
        words=mark.get('text');pages=[p for p in witness_pages(place) if all(p.get(k)==mark.get(k) for k in ('page','url','image_sha256'))]
        image=next((i for i in data.get('images') or [] if i.get('by')=='maker' and i.get('url')==mark.get('url') and i.get('sha256')==mark.get('image_sha256')),None)
        if not isinstance(words,str) or not words.strip() or '\n' in words or text.count(words)!=1 or len(str(mark.get('reason','')).strip())<30 or len(pages)!=1 or not image:problems.append('emphasis needs unique exact words, their maker page and a reason');continue
        if len(witness_pages(place))>1:
            span=place.get('page_span') if pages[0]==witness_pages(place)[0] else pages[0].get('span')
            if not isinstance(span,str) or words not in span:problems.append('emphasis must be wholly on the selected page')
        start=text.index(words);end=start+len(words)
        if any(start<b and a<end for a,b in used):problems.append('emphasis ranges must not overlap')
        used.append((start,end))
        if not offline:
            try:
                raw=Path(witness_cache(path,lang,image['file'])).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=image.get('sha256') or not valid_page_image(raw):problems.append('emphasis page changed')
            except (OSError,KeyError):problems.append('emphasis page is missing')
    if second:
        review=data.get('review') or {};reader=review.get('reader');check=(review.get('emphasis') or {}).get(loc) or {}
        if not reader or reader==(data.get('witness') or {}).get('maker') or check.get('reader')!=reader or check.get('sha256')!=page_number_digest(place) or len(str(check.get('assessment','')).strip())<60:problems.append('emphasis needs the independent page reader assessment')
    return problems

def cmd_witness_emphasis(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc) or die('no such place in this witness')
    if args.language=='ru' and modern_russian(place.get('text') or '')!=place.get('text'):die('normalize the Russian witness first so the displayed emphasis stays on the same words')
    pages=[p for p in witness_pages(place) if not args.page or p.get('page')==args.page]
    if len(pages)!=1:die('select exactly one printed page with --page')
    selected=pages[0];mark={'text':args.text,**{k:selected.get(k) for k in ('page','url','image_sha256')},'reason':args.reason}
    place.setdefault('emphasis',[]).append(mark);issues=witness_emphasis_problems(path,args.language,data,args.loc)
    if issues:die('; '.join(issues))
    data['review']={};save(file,data);print('printed emphasis recorded; independent page assessment required')

def cmd_witness_emphasis_review(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc) or {};review=data.get('review') or {};reader=review.get('reader')
    if not reader or reader!=args.reader or reader==(data.get('witness') or {}).get('maker') or not (review.get('places') or {}).get(args.loc) or len(args.assessment.strip())<60:die('the independent page reader must assess the printed emphasis')
    issues=witness_emphasis_problems(path,args.language,data,args.loc)
    if issues:die('; '.join(issues))
    review.setdefault('emphasis',{})[args.loc]={'reader':reader,'sha256':page_number_digest(place),'assessment':args.assessment};data['review']=review;save(file,data);print('independent emphasis assessment recorded')

def cmd_witness_emphasis_withdraw(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc) or {};marks=place.get('emphasis') or []
    matches=[item for item in marks if isinstance(item,dict) and item.get('text')==args.text]
    if len(matches)!=1 or len(args.reason.strip())<30:die('select one emphasis record and explain its withdrawal')
    marks.remove(matches[0]);data.setdefault('emphasis_corrections',[]).append({'loc':args.loc,'removed':matches[0],'review':data.get('review') or {},'reason':args.reason,'date':today()})
    if not marks:place.pop('emphasis',None)
    data['review']={};save(file,data);print('emphasis withdrawn with prior evidence; independent review required')

def witness_boundary_problems(path,lang,data,loc,second=False,offline=False):
    place=(data.get('places') or {}).get(loc) or {};proof=place.get('sentence_boundary') or {};text=place.get('text') or ''
    if not isinstance(proof,dict):return ['invalid sentence boundary evidence']
    suffix=proof.get('suffix','')
    source=next((item for item in data.get('sources') or [] if item.get('file')==place.get('source')),None)
    images=data.get('images') or [];maker_image=next((image for image in images if image.get('by')=='maker' and image.get('url')==proof.get('url') and image.get('sha256')==proof.get('image_sha256')),None)
    if not isinstance(text,str) or not isinstance(suffix,str) or not re.search(r'[”»"]\s*$',text) or not re.fullmatch(r'\s+\([^()\n]{1,120}\)[.!?]',suffix) or len(str(proof.get('reason','')).strip())<40 or not source or proof.get('source_sha256')!=source.get('sha256') or proof.get('url')!=place.get('url') or proof.get('image_sha256')!=place.get('image_sha256') or not maker_image:return ['sentence boundary needs the exact quoted text, following attribution and page evidence']
    if not offline:
        try:
            raw=Path(witness_cache(path,lang,source['file'])).read_bytes();image=Path(witness_cache(path,lang,maker_image['file'])).read_bytes();content=re.sub(r'\s+',' ',plain(raw,source['file']));run=re.sub(r'\s+',' ',text+suffix)
            if hashlib.sha256(raw).hexdigest()!=source.get('sha256') or run not in content or hashlib.sha256(image).hexdigest()!=proof.get('image_sha256') or not valid_page_image(image):return ['sentence boundary context or page image differs from the exact source']
        except (OSError,KeyError):return ['sentence boundary source is unavailable']
    if second:
        review=data.get('review') or {};reader=review.get('reader');answer=(review.get('boundaries') or {}).get(loc) or {}
        if not reader or reader==(data.get('witness') or {}).get('maker') or answer.get('reader')!=reader or answer.get('sha256')!=page_number_digest(place) or len(str(answer.get('assessment','')).strip())<60:return ['sentence boundary requires the independent page reader assessment']
    return []

def cmd_witness_sentence_boundary(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);place=(data.get('places') or {}).get(args.loc) or die('no such place in this witness')
    source=next((item for item in data.get('sources') or [] if item.get('file')==place.get('source')),None)
    if not source:die('record the exact source transcription first')
    place['sentence_boundary']={'suffix':args.suffix,'reason':args.reason,'source_sha256':source.get('sha256'),'url':place.get('url'),'image_sha256':place.get('image_sha256')}
    issues=witness_boundary_problems(path,args.language,data,args.loc)
    if issues:die('; '.join(issues))
    data['review']={};save(file,data);print('source sentence boundary recorded separately from the quoted text; independent review required')

def cmd_witness_boundary_review(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);review=data.get('review') or {};place=(data.get('places') or {}).get(args.loc) or {};reader=review.get('reader')
    if not reader or reader!=args.reader or reader==(data.get('witness') or {}).get('maker') or not (review.get('places') or {}).get(args.loc) or len(args.assessment.strip())<60:die('the independent page reader must assess the exact source boundary')
    issues=witness_boundary_problems(path,args.language,data,args.loc)
    if issues:die('; '.join(issues))
    review.setdefault('boundaries',{})[args.loc]={'reader':reader,'sha256':page_number_digest(place),'assessment':args.assessment}
    data['review']=review;save(file,data);print('independent sentence-boundary assessment recorded')

def cmd_witness_review(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if args.loc not in data['places']:die('no such place in this witness')
    place=data['places'][args.loc]
    pages=witness_pages(place)
    if any(not second_witness_image(data,page,args.reader) for page in pages):die('fetch or independently capture every same page as second reader first')
    review=data.setdefault('review',{})
    if review.get('reader') and review['reader']!=args.reader:die('a single second reader reviews this witness')
    review['reader']=args.reader;review['model']=args.model;review['date']=today();review.setdefault('places',{})[args.loc]={'reads':'as the packet','alignment':args.alignment,'image_sha256':place['image_sha256'],**({'pages':[{k:p.get(k) for k in ('page','url','image_sha256')} for p in pages]} if len(pages)>1 else {})}
    save(witness_file(path,args.language),data);print(f'{args.loc}: second reader recorded')

def collective_translation_problems(path,lang,data,second=False,offline=False):
    witness=data.get('witness') or {};proof=witness.get('collective_translation') or {};members=proof.get('members') if isinstance(proof,dict) else None;places=proof.get('places') if isinstance(proof,dict) else None
    if not isinstance(members,list) or len(members)<2 or any(not isinstance(name,str) or len(name.strip())<3 for name in members) or len(set(name.casefold() for name in members))!=len(members) or not isinstance(places,dict) or set(places)!=set((data.get('places') or {}).keys()):return ['collective translation needs distinct named source authors and evidence for every place']
    problems=[];attribution=str(witness.get('attribution') or '').casefold();rights=witness.get('rights') or {}
    if any(name.casefold() not in attribution for name in members):problems.append('collective attribution must name every source author without naming individual translators')
    if not isinstance(rights,dict) or any(not isinstance(rights.get(c),dict) or rights[c].get('basis') not in ('licensed','short-quotation') for c in JURISDICTIONS):problems.append('unknown collective contributors cannot establish public domain by dates')
    for loc,claim in places.items():
        place=(data.get('places') or {}).get(loc) or {};source=next((s for s in data.get('sources') or [] if isinstance(claim,dict) and all(s.get(k)==claim.get(k) for k in ('file','url','sha256'))),None)
        if not source or not isinstance(source.get('file'),str) or not source['file'] or not lawful_url(source.get('url')) or not re.fullmatch(r'[a-f0-9]{64}',str(source.get('sha256',''))) or not isinstance(claim.get('claim'),str) or len(claim['claim'].strip())<5 or not isinstance(claim.get('assessment'),str) or len(claim['assessment'].strip())<60:problems.append(f'{loc}: collective attribution needs the exact source, statement and assessment');continue
        if not offline:
            try:
                raw=Path(witness_cache(path,lang,source['file'])).read_bytes();content=squeeze(plain(raw,source['file']));needle=squeeze(str(place.get('text') or '')).strip('“”"«»');statement=claim['claim'];positions=[m.start() for m in re.finditer(re.escape(needle),content)] if needle else []
                if hashlib.sha256(raw).hexdigest()!=source.get('sha256') or any(name.casefold() not in content.casefold() for name in members) or len(positions)!=1 or statement.casefold() not in content[positions[0]:positions[0]+len(needle)+300].casefold():problems.append(f'{loc}: collective source must name the authors and attribute this exact translation nearby')
            except (OSError,KeyError,TypeError):problems.append(f'{loc}: collective source is unavailable')
        if second:
            review=data.get('review') or {};reader=review.get('reader');check=(review.get('collectives') or {}).get(loc) or {};scope={'members':members,'claim':claim,'place':place,'edition':witness.get('edition'),'attribution':witness.get('attribution'),'rights':rights}
            if not reader or reader==witness.get('maker') or check.get('reader')!=reader or check.get('sha256')!=page_number_digest(scope) or len(str(check.get('assessment','')).strip())<60:problems.append(f'{loc}: collective attribution needs independent source and rights assessment')
    return problems

def cmd_witness_collective(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);spec=load(args.file)
    if not isinstance(spec,dict) or set(spec)!={'members','source','claim','assessment'}:die('give collective members, source filename, its attribution statement and assessment')
    source=next((s for s in data.get('sources') or [] if s.get('file')==spec['source']),None)
    if not source or args.loc not in (data.get('places') or {}):die('record this source and place first')
    witness=data['witness'];old=witness.get('collective_translation') or {};members=spec['members']
    if old and old.get('members')!=members:die('keep the same source collective in this witness')
    proof={'members':members,'places':dict(old.get('places') or {})};proof['places'][args.loc]={**{k:source.get(k) for k in ('file','url','sha256')},'claim':spec['claim'],'assessment':spec['assessment']};witness['collective_translation']=proof
    # Additional places can be added one by one; validate only the recorded scope here.
    partial={**data,'places':{loc:data['places'][loc] for loc in proof['places']}};issues=collective_translation_problems(path,args.language,partial)
    if issues:die('; '.join(issues))
    if old and old!=proof:data.setdefault('collective_corrections',[]).append({'removed':old,'review':data.get('review') or {},'reason':'Collective source evidence updated; independent review must be renewed.','date':today()})
    data['review']={};save(witness_file(path,args.language),data);print('collective source attribution recorded; individual translators remain unidentified')

def cmd_witness_collective_review(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);witness=data.get('witness') or {};proof=witness.get('collective_translation') or {};claim=(proof.get('places') or {}).get(args.loc);place=(data.get('places') or {}).get(args.loc);review=data.get('review') or {};reader=review.get('reader')
    if not claim or not place or not reader or reader!=args.reader or reader==witness.get('maker') or not (review.get('places') or {}).get(args.loc) or len(args.assessment.strip())<60:die('the independent page reader must assess the collective source and its rights')
    if collective_translation_problems(path,args.language,data):die('complete the source-bound collective evidence for every place first')
    scope={'members':proof['members'],'claim':claim,'place':place,'edition':witness.get('edition'),'attribution':witness.get('attribution'),'rights':witness.get('rights') or {}}
    review.setdefault('collectives',{})[args.loc]={'reader':reader,'sha256':page_number_digest(scope),'assessment':args.assessment};data['review']=review;save(witness_file(path,args.language),data);print('independent collective attribution assessment recorded')

def cmd_witness_collective_clear(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);witness=data.get('witness') or {};old=witness.get('collective_translation')
    if not old or len(args.reason.strip())<40:die('record a collective attribution and a detailed reason for withdrawal')
    data.setdefault('collective_corrections',[]).append({'removed':old,'review':data.get('review') or {},'reason':args.reason,'date':today()});witness.pop('collective_translation');data['review']={};save(witness_file(path,args.language),data)
    print('collective attribution withdrawn with its history; renew review')

def anonymous_digest(proof):
    return hashlib.sha256(json.dumps(proof,sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()

def unidentified_translator_problems(path,lang,data,second=False,offline=False):
    witness=data.get('witness') or {};proof=witness.get('unidentified_translator');problems=[]
    if not isinstance(proof,dict):return ['unidentified translator evidence must be an object']
    if proof.get('edition')!=witness.get('edition') or len(str(proof.get('finding','')).strip())<60:problems.append('unidentified translator needs the exact edition and a documented identity search')
    searches=proof.get('searches') or []
    if not isinstance(searches,list) or len({item.get('url') for item in searches if isinstance(item,dict) and lawful_url(item.get('url')) and len(str(item.get('outcome','')).strip())>=20})<2:problems.append('unidentified translator needs two distinct documented identity searches')
    references=proof.get('sources') or [];known=data.get('sources') or []
    if not isinstance(references,list) or not references:problems.append('unidentified translator needs a fetched exact-edition source');references=[]
    for ref in references:
        if not isinstance(ref,dict) or not any(all(source.get(k)==ref.get(k) for k in ('file','url','sha256')) for source in known) or not re.fullmatch(r'[a-f0-9]{64}',str(ref.get('sha256',''))):problems.append('unidentified translator source is not a recorded fetched source');continue
        if not offline:
            try:
                if hashlib.sha256(Path(witness_cache(path,lang,ref['file'])).read_bytes()).hexdigest()!=ref['sha256']:problems.append('unidentified translator source changed')
            except (OSError,KeyError,TypeError):problems.append('unidentified translator source is unavailable')
    rights=witness.get('rights') or {}
    if not isinstance(rights,dict):rights={}
    if any(not isinstance(rights.get(country),dict) or rights[country].get('basis')!='short-quotation' for country in JURISDICTIONS):problems.append('an unidentified translator here is permitted only on separately assessed short-quotation bases')
    if second:
        review=data.get('review') or {};reader=review.get('reader');check=review.get('unidentified_translator') or {}
        if not reader or reader==witness.get('maker') or check.get('reader')!=reader or check.get('sha256')!=page_number_digest(proof) or len(str(check.get('assessment','')).strip())<60:problems.append('unidentified translator needs independent source and identity assessment')
    return problems

def cmd_witness_unidentified_translator(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);info=load(args.file);witness=data['witness']
    if not isinstance(info,dict) or set(info)!={'finding','searches','sources'} or not isinstance(info.get('sources'),list):die('provide finding, documented searches and fetched source filenames')
    if witness.get('anonymous_translation') or any(isinstance(p,dict) and p.get('role')=='translator' for p in witness.get('persons') or []):die('clear any identified or anonymous-publication translator claim first')
    refs=[]
    for name in info['sources']:
        source=next((s for s in data.get('sources') or [] if s.get('file')==name),None)
        if not source:die(f'fetch identity source {name} first')
        refs.append({key:source.get(key) for key in ('file','url','sha256')})
    witness['unidentified_translator']={**info,'sources':refs,'edition':witness.get('edition')};issues=unidentified_translator_problems(path,args.language,data)
    if issues:die('; '.join(issues))
    data['review']={};save(file,data);print('unidentified translation hand recorded; independent source assessment required')

def cmd_witness_unidentified_review(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);review=data.get('review') or {};reader=review.get('reader');proof=(data.get('witness') or {}).get('unidentified_translator')
    if not proof or not review.get('places') or not reader or reader!=args.reader or reader==(data.get('witness') or {}).get('maker') or len(args.assessment.strip())<60:die('the independent page reader must assess the identity search and exact source')
    issues=unidentified_translator_problems(path,args.language,data)
    if issues:die('; '.join(issues))
    review['unidentified_translator']={'reader':reader,'sha256':page_number_digest(proof),'assessment':args.assessment};data['review']=review;save(file,data);print('independent unidentified translator assessment recorded')

def cmd_witness_unidentified_clear(args):
    path=packet(args.packet);file=witness_file(path,args.language)
    if os.path.isfile(file+'.accepted'):die('an accepted witness cannot be changed')
    data=witness_data(path,args.language);proof=(data.get('witness') or {}).pop('unidentified_translator',None)
    if not proof or len(args.reason.strip())<30:die('record an existing unidentified hand and a reason to clear it')
    data.setdefault('translator_corrections',[]).append({'removed':proof,'review':data.get('review') or {},'reason':args.reason,'date':today()});data['review']={};save(file,data);print('unidentified hand retired with its history; renew review')

def anonymous_problems(path,lang,data,second=False,offline=False):
    """Review an anonymously published translation, not an unknown named person."""
    witness=data.get('witness') or {};proof=witness.get('anonymous_translation');problems=[]
    if not isinstance(proof,dict):return ['anonymous translation evidence must be an object']
    if proof.get('edition')!=witness.get('edition'):problems.append('anonymous translation needs the same exact edition')
    year=proof.get('first_publication_year');edition_year=witness.get('year')
    if type(year) is not int or year<1 or type(edition_year) is not int or year>edition_year:problems.append('anonymous translation needs the first lawful publication year, no later than this edition')
    for field in ('lawful_publication','identity_research','same_version'):
        if len(str(proof.get(field,'')).strip())<60:problems.append(f'anonymous translation needs a detailed {field} assessment')
    searches=proof.get('searches') or []
    if not isinstance(searches,list):searches=[]
    valid_searches=[item for item in searches if isinstance(item,dict) and lawful_url(item.get('url')) and len(str(item.get('outcome','')).strip())>=20]
    if len({item['url'] for item in valid_searches})<3:problems.append('anonymous translation needs at least three documented identity searches')
    references=proof.get('sources') or [];known=data.get('sources') or []
    if not isinstance(references,list):references=[]
    if len({item.get('url') for item in references if isinstance(item,dict) and lawful_url(item.get('url'))})<2:problems.append('anonymous translation needs two distinct fetched sources')
    for ref in references:
        if not isinstance(ref,dict) or not any(all(source.get(k)==ref.get(k) for k in ('file','url','sha256')) for source in known) or not re.fullmatch(r'[a-f0-9]{64}',str(ref.get('sha256',''))):
            problems.append('anonymous translation source is not the recorded fetched source');continue
        if not offline:
            try:
                raw=Path(witness_cache(path,lang,ref['file'])).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=ref['sha256']:problems.append('anonymous translation source changed')
            except (OSError,KeyError,TypeError):problems.append('anonymous translation source is unavailable')
    if second:
        review=data.get('review') or {};check=review.get('anonymous') or {};reader=review.get('reader')
        if not reader or reader==witness.get('maker') or check.get('reader')!=reader or check.get('sha256')!=anonymous_digest(proof) or len(str(check.get('assessment','')).strip())<60:
            problems.append('anonymous translation needs an independent assessment of the same publication, version and identity search')
    return problems

def cmd_witness_anonymous(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);info=load(args.file)
    if not isinstance(info,dict) or set(info)!={'first_publication_year','lawful_publication','identity_research','same_version','searches','sources'}:die('anonymous evidence needs the documented publication, identity research, version, searches and source filenames')
    refs=[]
    if not isinstance(info.get('sources'),list):die('anonymous sources must be filenames of fetched sources')
    for filename in info['sources']:
        source=next((entry for entry in data.get('sources') or [] if entry.get('file')==filename),None)
        if not source:die(f'fetch anonymous-publication source {filename} first')
        refs.append({key:source.get(key) for key in ('file','url','sha256')})
    proof={**info,'sources':refs,'edition':data['witness'].get('edition')}
    data['witness']['anonymous_translation']=proof;data['review']={}
    issues=anonymous_problems(path,args.language,data,offline=False)
    if issues:die('anonymous evidence is incomplete: '+'; '.join(issues))
    save(witness_file(path,args.language),data);print('anonymous-publication evidence recorded; independent assessment required')

def cmd_witness_anonymous_review(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language);review=data.get('review') or {};reader=review.get('reader');proof=(data.get('witness') or {}).get('anonymous_translation')
    if not proof or not reader or reader==(data.get('witness') or {}).get('maker'):die('record the anonymous evidence and a different second reader first')
    if len(args.assessment.strip())<60:die('assess the edition, lawful publication and identity search in detail')
    review['anonymous']={'reader':reader,'sha256':anonymous_digest(proof),'assessment':args.assessment};data['review']=review
    save(witness_file(path,args.language),data);print('independent anonymous-publication assessment recorded')

def cmd_witness_anonymous_clear(args):
    path=packet(args.packet);data=editable_witness_data(path,args.language)
    if not (data.get('witness') or {}).pop('anonymous_translation',None):die('no anonymous-publication evidence is recorded')
    data['review']={};save(witness_file(path,args.language),data);print('anonymous claim cleared; record the identified translator and renew review')

def witness_public_domain_by_dates(people,year):
    dates=public_domain_by_dates(people,year)
    # An anonymous original's first lawful publication and any posthumous term
    # are separate evidence; a historical death bound alone cannot establish them.
    if any(p.get('role')=='author' and p.get('anonymous') for p in people):dates.update({'russia':False,'eu':False})
    return dates

def witness_problems(path,lang,data,second=False,offline=False):
    problems=[];add=problems.append;witness=data.get('witness') or {};places=data.get('places') or {}
    problems.extend(witness_withdrawal_problems(path,lang))
    problems.extend(derived_source_problems(data.get('sources') or [],lambda name:witness_cache(path,lang,name),offline,images=data.get('images') or []))
    marker=witness_file(path,lang)+'.accepted';file=witness_file(path,lang)
    legacy=os.path.isfile(file) and os.path.isfile(marker) and load(marker).get('sha256')==hashlib.sha256(Path(file).read_bytes()).hexdigest()
    problems.extend(derived_image_problems(data.get('sources') or [],data.get('images') or [],lambda name:witness_cache(path,lang,name),offline,legacy=legacy))
    for image in data.get('images') or []:
        if image.get('capture'):problems.extend(browser_capture_problems(path,lang,data,image,offline=offline))
    if data.get('language')!=lang:add('language differs from filename')
    rights=witness.get('rights') or {}
    if not isinstance(rights,dict):add('witness.rights must be an object');rights={}
    basis=lambda country:rights[country].get('basis') if isinstance(rights.get(country),dict) else None
    quoted=any(basis(country)=='short-quotation' for country in JURISDICTIONS)
    if quoted or witness.get('mexico_quotation'):
        issues=mexico_problems(witness,places,data.get('review') or {},witness.get('maker'),second,path=path,lang=lang,offline=offline)
        file=witness_file(path,lang);marker=file+'.accepted'
        legacy=os.path.isfile(file) and os.path.isfile(marker) and load(marker).get('sha256')==hashlib.sha256(Path(file).read_bytes()).hexdigest()
        for issue in issues:
            if legacy and (not witness.get('mexico_quotation') or issue=='Mexico quotation assessment needs an explicit review outcome'):OWES.append(f'{lang}: {issue}')
            else:problems.append(issue)
    for field in ('edition','attribution','licence','url'):
        if not isinstance(witness.get(field),str) or not witness[field].strip():add(f'witness.{field} is missing')
    if not lawful_url(witness.get('url','')):add('witness.url must be a lawful https URL')
    recorded_people=witness.get('persons') or []
    if not isinstance(recorded_people,list):add('witness.persons must be a list of sourced participants');recorded_people=[]
    people=[person for person in recorded_people if isinstance(person,dict)]
    if len(people)!=len(recorded_people):add('unsupported provisional participant needs source evidence or recorded removal')
    if not witness.get('maker'):add('witness.maker is missing')
    identified_translator=any(p.get('role')=='translator' for p in people)
    anonymous=witness.get('anonymous_translation');unidentified=witness.get('unidentified_translator');collective=witness.get('collective_translation')
    if not identified_translator and not anonymous and not unidentified and not collective:add('translator is missing')
    if sum(bool(value) for value in (identified_translator,anonymous,unidentified,collective))>1:add('retain exactly one identified, collective, anonymous-publication or unidentified translator claim')
    if collective:problems.extend(collective_translation_problems(path,lang,data,second,offline))
    if unidentified:problems.extend(unidentified_translator_problems(path,lang,data,second,offline))
    anonymous_issues=anonymous_problems(path,lang,data,second,offline) if anonymous else []
    problems.extend(anonymous_issues)
    if not any(p.get('role')=='author' for p in people):add('author is missing')
    original_authors=[p for p in (evidence(path).get('persons') or []) if p.get('role')=='author']
    w=world()
    for author in layer_authors(w,work_of(path)):
        if not any(same_participant(author,person) for person in original_authors):original_authors.append(author)
    for author in original_authors:
        if not any(person.get('role')=='author' and same_participant(author,person) for person in people):add('every original author must also be recorded in the translation')
    if any((p.get('died') is not None and type(p.get('died')) is not int) or not p.get('source') or not str(p.get('source')).startswith('https://') for p in people):add('each hand needs a sourced date or an unknown death year')
    for person in people:
        if person.get('anonymous') and person.get('role')!='author':add('an anonymous original author cannot stand in for a named translator or editor')
        if person.get('wartime') is not None and (type(person['wartime']) is not bool or not trusted(person.get('wartime_source'))):add('wartime status requires a trusted source')
        if person.get('rehabilitated') is not None and (type(person['rehabilitated']) is not int or not trusted(person.get('rehabilitation_source'))):add('rehabilitation requires a year and a trusted source')
        evidence_record=person.get('evidence') or {};qid=person.get('qid');reference=evidence_record.get('kind')=='reference';publisher=evidence_record.get('kind')=='publisher-identity';image_identity=evidence_record.get('kind')=='image-identity';creator_identity=evidence_record.get('kind')=='creator-declaration'
        if publisher:problems.extend(publisher_identity_problems(path,lang,data,person,second=second,offline=offline))
        if image_identity:problems.extend(image_identity_problems(path,lang,data,person,second=second,offline=offline))
        if creator_identity:
            problems.extend(creator_identity_problems(path,lang,data,person,offline=offline))
            if not witness.get('created_translation'):add('a creator declaration is only for an explicitly new translation')
        if reference:problems.extend(reference_identity_problems(data,person,second))
        if qid:problems.extend(witness_linked_bound_problems(path,lang,person,offline=offline))
        if (reference or person.get('bound_reference') or (person.get('death_bound') or {}).get('from')=='wikidata') and person.get('died_before') is not None:
            bound=person.get('death_bound') or {}
            if not isinstance(bound,dict) or type(person['died_before']) is not int or person['died_before']>BOUND_BEFORE or person.get('died') is not None or bound.get('year')!=person['died_before'] or (bound.get('from')!='wikidata' and (bound.get('from')!=(person.get('bound_reference') or {}).get('url',person.get('source')) or len(str(bound.get('note','')).strip())<30)):add('sourced death bound needs its trusted reference, uncertainty and detailed assessment')
            if second:
                assessment=((data.get('review') or {}).get('bounds') or {}).get(page_number_digest(person)) or {}
                if assessment.get('reader')!=(data.get('review') or {}).get('reader') or not assessment.get('reader') or assessment.get('reader')==witness.get('maker') or len(str(assessment.get('assessment','')).strip())<60:add('sourced death bound requires independent reference assessment')
        valid_digest=bool(re.fullmatch(r'[a-f0-9]{64}',str(evidence_record.get('sha256',''))))
        valid_url=(lawful_url(person.get('source')) and evidence_record.get('url')==person.get('source')) if publisher or image_identity or creator_identity else (trusted(person.get('source')) and evidence_record.get('url')==person.get('source')) if reference else (bool(re.fullmatch(r'Q[1-9]\d*',str(qid or ''))) and evidence_record.get('url')==f'https://www.wikidata.org/wiki/Special:EntityData/{qid}.json')
        if not valid_url or not valid_digest:
            add('each hand needs fetched evidence from witness-person or witness-person-source');continue
        if not offline:
            try:
                raw=Path(witness_cache(path,lang,evidence_record['file'])).read_bytes()
                if hashlib.sha256(raw).hexdigest()!=evidence_record['sha256']:add(f'{qid or person.get("name")}: person evidence changed');continue
                if publisher or image_identity or creator_identity:pass  # checked against the exact source above
                elif reference:
                    content=plain(raw,evidence_record['file']).casefold();name=(person.get('name') or {}).get('en','')
                    if not name or name.casefold() not in content or any(year is not None and str(year) not in content for year in (person.get('born'),person.get('died'))):add(f'{name}: stated name or dates are absent from the fetched reference')
                else:
                    entity=json.loads(raw)['entities'][qid]
                    for field,prop in (('born','P569'),('died','P570')):
                        if person.get(field)!=entity_year(entity,prop):add(f'{qid}: {field} year differs from the fetched entity or is not precise enough')
                    if person.get('died_before') is not None and not person.get('bound_reference') and (death_bound(entity) or {}).get('year')!=person['died_before']:add(f'{qid}: the upper bound of the death differs from the fetched entity')
                    labels={value.get('value','').casefold() for value in entity.get('labels',{}).values()}
                    if not any(str(value).casefold() in labels for value in (person.get('name') or {}).values() if value):add(f'{qid}: name differs from the fetched entity')
            except (OSError,KeyError,ValueError,TypeError):add(f'{qid or person.get("name")}: person evidence is missing or invalid')
    problems.extend(license_problems(witness,data.get('sources') or [],lambda name:witness_cache(path,lang,name),data.get('review') or {},witness.get('maker'),second,offline))
    dates=witness_public_domain_by_dates(people,witness.get('year'))
    if anonymous:
        # The EU anonymous term applies to this translation; identified authors
        # and editors retain their own death-based terms. Russia needs its own evidence.
        first=anonymous.get('first_publication_year') if isinstance(anonymous,dict) else None
        dates['eu']=dates['eu'] and not anonymous_issues and type(first) is int and first<=datetime.date.today().year-71
        dates['russia']=False
    for country in JURISDICTIONS:
        item=rights.get(country) or {}
        if not isinstance(item,dict) or item.get('basis') not in BASES or len(str(item.get('law','')))<20:add(f'rights.{country} needs basis and law')
        elif item['basis'] in ('public-domain','public-domain+licence') and not dates[country]:add(f'rights.{country} is not established by dates')
    if lang=='ru' and len(str(witness.get('normalization','')))<20:add('Russian witness needs a normalization provenance statement')
    original={**((world()['fragments'].get('places') or {}).get(work_of(path),{})),**((part(path,'places.json') or {}).get('places') or {})}
    if not places:add('no translated places')
    if places and any(basis(country)=='locator-only' for country in JURISDICTIONS):add('translated text cannot be distributed while a jurisdiction permits a locator only')
    sources={item.get('file'):item for item in data.get('sources') or []}
    for loc,place in places.items():
        if loc not in original:add(f'{loc}: no original place');continue
        text=place.get('text') or ''
        quote=any(basis(country)=='short-quotation' for country in JURISDICTIONS)
        if 'emphasis' in place:problems.extend(f'{loc}: {issue}' for issue in witness_emphasis_problems(path,lang,data,loc,second=second,offline=offline))
        boundary=place.get('sentence_boundary');boundary_issues=witness_boundary_problems(path,lang,data,loc,second=second,offline=offline) if boundary else []
        problems.extend(f'{loc}: {issue}' for issue in boundary_issues)
        sentence_text=text+(boundary.get('suffix','') if isinstance(boundary,dict) and not boundary_issues else '')
        for problem in sentences(sentence_text,limit=None if quote or (boundary and not boundary_issues) else LONGEST):add(f'{loc}: {problem}')
        if boundary and not boundary_issues and not quote and len(text)>LONGEST:add(f'{loc}: {len(text)} characters: a place is the sentences that say the thing, at most {LONGEST} characters')
        if len(str(place.get('alignment','')))<20:add(f'{loc}: explain the alignment to the original')
        source=sources.get(place.get('source'));transcription=place.get('transcription') or {}
        if not isinstance(transcription,dict):add(f'{loc}: invalid transcription provenance');transcription={}
        human=transcription.get('kind')=='human-page';created=transcription.get('kind')=='new-translation'
        if human:
            if place.get('source') or len(str(transcription.get('reason','')).strip())<40:add(f'{loc}: human page transcription needs its reason and no claimed downloaded transcription')
        elif created:
            if place.get('source'):add(f'{loc}: a new translation must not claim a downloaded transcription')
            problems.extend(f'{loc}: {issue}' for issue in new_translation_problems(path,lang,data,loc,second=second,offline=offline))
        elif not source or not lawful_url(source.get('url','')):add(f'{loc}: no lawful source');continue
        if witness.get('created_translation') and not created:add(f'{loc}: a newly created witness must distinguish its own translation from a fetched edition')
        if not place.get('page') or not lawful_url(place.get('url','')):add(f'{loc}: page or exact address and lawful URL are required')
        if place.get('address') is not None and (not valid_address(place['address']) or place['address'].get('url')!=place.get('url')):add(f'{loc}: structured address must name the same lawful source')
        line_locator=place.get('line_locator')
        if line_locator is not None and (not isinstance(line_locator,dict) or not {'start','end'}<=set(line_locator) or set(line_locator)-{'start','end','section'} or type(line_locator.get('start')) is not int or type(line_locator.get('end')) is not int or line_locator['start']<1 or line_locator['end']<line_locator['start'] or ('section' in line_locator and (not isinstance(line_locator['section'],str) or not line_locator['section'].strip() or len(line_locator['section'])>100)) or not re.fullmatch(r'\s*(?:p(?:age)?\.?\s*)?\d+[a-zA-Z]?\s*',str(place.get('page') or ''))):add(f'{loc}: line locator needs ascending integer bounds and one separate printed page')
        extra=place.get('extra_pages') or []
        if not isinstance(extra,list) or any(not isinstance(page,dict) for page in extra):add(f'{loc}: invalid additional pages');extra=[]
        pages=[{'page':place.get('page'),'url':place.get('url'),'image_sha256':place.get('image_sha256')},*extra]
        if source and (source.get('derived_from') or {}).get('tool')=='tesseract-image' and not any(page.get('image_sha256')==source['derived_from'].get('sha256') and page.get('url')==source.get('url') for page in pages):add(f'{loc}: OCR must be bound to its exact source page')
        if source and (source.get('derived_from') or {}).get('tool')=='ocr-page-join':
            parents=[sources.get(segment.get('file')) for segment in source['derived_from'].get('segments') or [] if isinstance(segment,dict)]
            required=[((p.get('derived_from') or {}).get('sha256'),p.get('url')) for p in parents if p]
            if len(required)!=len(parents) or required!=[(page.get('image_sha256'),page.get('url')) for page in pages]:add(f'{loc}: joined OCR requires every source page in the same order')
            if second:
                review=data.get('review') or {};check=(review.get('ocr_joins') or {}).get(loc) or {}
                if not review.get('reader') or review.get('reader')==witness.get('maker') or check.get('reader')!=review.get('reader') or check.get('sha256')!=page_number_digest({'source':source,'place':place}) or len(str(check.get('assessment','')).strip())<60:add(f'{loc}: joined OCR needs an independent page-join and omission assessment')
        page_range=re.search(r'\b(\d+)\s*[-–—]\s*(\d+)\b',str(place.get('page') or ''))
        if page_range and int(page_range.group(2))>=int(page_range.group(1)) and len(pages)<int(page_range.group(2))-int(page_range.group(1))+1:add(f'{loc}: a page range needs an image for every printed page')
        if len({p.get('page') for p in pages})!=len(pages):add(f'{loc}: duplicate printed page')
        for page in pages:
            if any(other.get('image_sha256')==page.get('image_sha256') and other.get('url')!=page.get('url') for other in pages):add(f'{loc}: a shared spread must retain the same source URL');break
        for page in extra:
            if not page.get('page') or not lawful_url(page.get('url','')) or not page.get('span') or page['span'] not in text:add(f'{loc}: additional page needs its address and words from that page')
        if extra and (not place.get('page_span') or re.sub(r'\s+','',str(place.get('page_span') or '')+''.join(str(page.get('span') or '') for page in extra))!=re.sub(r'\s+','',text)):add(f'{loc}: page spans must cover the complete place in order')
        images=[next((item for item in data.get('images') or [] if item.get('by')=='maker' and item.get('sha256')==page.get('image_sha256') and item.get('url')==page.get('url')),None) for page in pages]
        if any(image is None for image in images):add(f'{loc}: maker page image is missing')
        back=text
        for correction in reversed(place.get('corrections') or []):
            old,new=correction.get('from'),correction.get('to')
            if not isinstance(old,str) or not old or not isinstance(new,str) or not new or old==new or len(str(correction.get('reason','')))<30 or not any(all(correction.get(k)==page.get(k) for k in ('url','page','image_sha256')) for page in pages):
                add(f'{loc}: correction needs both readings, a reason and the same page image');continue
            offset=correction.get('offset')
            if type(offset) is not int or offset<0 or back[offset:offset+len(new)]!=new:add(f'{loc}: correction offset no longer identifies the corrected run');continue
            back=back[:offset]+old+back[offset+len(new):]
        if human or created:
            original_text=transcription.get('translation_text') if created else transcription.get('original_text')
            if not isinstance(original_text,str) or not original_text.strip():add(f'{loc}: human transcription or new translation needs its originally recorded words')
            elif back!=(modern_russian(original_text,old_source=old_russian_source(original_text)) if human and lang=='ru' else original_text):add(f'{loc}: text differs from the originally recorded words')
        if not offline:
            for record in [source,*images]:
                if not record:continue
                try:
                    raw=Path(witness_cache(path,lang,record['file'])).read_bytes()
                    if hashlib.sha256(raw).hexdigest()!=record.get('sha256'):add(f"{loc}: {record['file']} changed")
                    elif record in images and not valid_page_image(raw):add(f'{loc}: maker page image is invalid')
                    elif record is source:
                        if raw.startswith(b'%PDF-'):add(f'{loc}: derive the PDF text before cutting a witness')
                        else:
                            problem=in_transcription(back,modern_russian(plain(raw,record['file'])) if lang=='ru' else plain(raw,record['file']))
                            if problem:add(f'{loc}: {problem}')
                except (OSError,KeyError):add(f'{loc}: local source or image is missing')
        if quote:
            if not place.get('lines'):add(f'{loc}: quotation lines are missing')
            count=len(re.findall(r'(?:[.!?…]+(?:[”»’」』\"\')\]]+)?(?=\s|$)|[。！？؟।॥]+[”»’」』\"\')\]]*)',text))
            if re.search(r'[\u0370-\u03ff\u1f00-\u1fff]',text):count+=text.count(';')
            if boundary and not boundary_issues:count+=1
            if (len(text)>900 or count>3) and len(str(place.get('justification','')))<30:add(f'{loc}: quotation exceeds the default and needs a justification')
        if second:
            review=data.get('review') or {};answer=(review.get('places') or {}).get(loc) or {}
            if not all(review.get(field) for field in ('reader','model','date')) or review.get('reader')==witness.get('maker'):add(f'{loc}: a different second reader with model and date is required')
            if answer.get('reads')!='as the packet' or answer.get('image_sha256')!=place.get('image_sha256') or len(str(answer.get('alignment','')))<20:add(f'{loc}: second reader must verify page and alignment')
            if len(pages)>1 and answer.get('pages')!=[{k:p.get(k) for k in ('page','url','image_sha256')} for p in pages]:add(f'{loc}: second reader must review every page of this place')
            for page in pages:
                second_image=second_witness_image(data,page,review.get('reader'))
                if not second_image:add(f'{loc}: second reader page image is missing')
                elif not offline:
                    try:
                        second_raw=Path(witness_cache(path,lang,second_image['file'])).read_bytes()
                        if hashlib.sha256(second_raw).hexdigest()!=second_image.get('sha256') or not valid_page_image(second_raw):add(f'{loc}: second reader image changed or is invalid')
                    except (OSError,KeyError):add(f'{loc}: second reader image is missing locally')
    return problems

def cmd_witness_gate(args):
    path=packet(args.packet);data=witness_data(path,args.language)
    OWES.clear()
    problems=witness_problems(path,args.language,data,second=args.second,offline=args.offline)
    if args.accept and args.second and not args.offline:
        conflict=witness_conflict(path,args.language,data)
        if conflict:problems.append(conflict)
    for problem in problems:print(problem)
    for debt in OWES:print('owes: '+debt)
    print('gate: '+('shut' if problems else 'open'))
    if problems:sys.exit(1)
    if args.accept:
        if not args.second or args.offline:die('accept requires the online second gate')
        file=witness_file(path,args.language);raw=Path(file).read_bytes()
        save(file+'.accepted',{'sha256':hashlib.sha256(raw).hexdigest(),'date':today()})
        print('witness accepted')


def main():
    p=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter);s=p.add_subparsers(dest='cmd',required=True)
    def sub(name,fn,*arguments):
        q=s.add_parser(name);q.set_defaults(fn=fn)
        for names,options in arguments:q.add_argument(*names,**options)
    P=(('packet',),{})
    sub('extension-open',cmd_extension_open,P,(('name',),{}),(('--keys',),{'required':True}));
    sub('witness-extension-open',cmd_witness_extension_open,P,(('name',),{}));
    sub('scan-rebind',cmd_scan_rebind,P,(('loc',),{}),(('item',),{}),(('--image',),{'required':True}),(('--reason',),{'required':True}));
    sub('import',cmd_import,P,(('part',),{'choices':['places','reading','review','blocked']}),(('file',),{}));sub('unblock',cmd_unblock,P)
    sub('witness-parent',cmd_witness_parent,(('work',),{}));
    sub('language-search',cmd_language_search,P,(('language',),{}),(('file',),{}))
    sub('language-search-review',cmd_language_search_review,P,(('language',),{}),(('--reader',),{'required':True}),(('--model',),{'required':True}),(('--assessment',),{'required':True}))
    sub('language-search-accept',cmd_language_search_accept,P,(('language',),{}))
    sub('language-search-withdraw',cmd_language_search_withdraw,P,(('language',),{}))
    sub('mexico-quotation',cmd_mexico_quotation,P,(('--language',),{}),(('--non-substantial',),{'required':True}),(('--normal-exploitation',),{'required':True}),(('--source-integrity',),{'required':True}),(('--law-file',),{}))
    sub('mexico-quotation-review',cmd_mexico_quotation_review,P,(('--language',),{}),(('--assessment',),{'required':True}),(('--outcome',),{'choices':['approved','rejected','pending'],'required':True}))
    sub('witness-open',cmd_witness_open,P,(('language',),{}),(('metadata',),{}))
    sub('witness-info',cmd_witness_info,P,(('language',),{}),(('file',),{}));
    sub('witness-rights',cmd_witness_rights,P,(('language',),{}),(('jurisdiction',),{'choices':JURISDICTIONS}),(('basis',),{'choices':sorted(BASES)}),(('law',),{}));
    sub('licence',cmd_licence,P,(('source',),{}),(('url',),{}),(('explanation',),{}),(('--language',),{}));
    sub('licence-review',cmd_licence_review,P,(('assessment',),{}),(('--language',),{}));
    sub('witness-browser-capture',cmd_witness_browser_capture,P,(('language',),{}),(('source',),{}),(('--file',),{'required':True}),(('--by',),{'choices':['maker','second'],'required':True}),(('--reader',),{'required':True}),(('--captured-at',),{'required':True}))
    sub('original-browser-capture',cmd_original_browser_capture,P,(('loc',),{}),(('source',),{}),(('--file',),{'required':True}),(('--by',),{'choices':['maker','second'],'required':True}),(('--reader',),{'required':True}),(('--captured-at',),{'required':True}),(('--n',),{'type':int,'required':True}),(('--address-kind',),{'choices':['section','chapter','paragraph'],'required':True}),(('--address-value',),{'required':True}))
    sub('witness-sentence-boundary',cmd_witness_sentence_boundary,P,(('language',),{}),(('loc',),{}),(('--suffix',),{'required':True}),(('--reason',),{'required':True}))
    sub('witness-boundary-review',cmd_witness_boundary_review,P,(('language',),{}),(('loc',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-emphasis',cmd_witness_emphasis,P,(('language',),{}),(('loc',),{}),(('--text',),{'required':True}),(('--page',),{}),(('--reason',),{'required':True}))
    sub('witness-emphasis-review',cmd_witness_emphasis_review,P,(('language',),{}),(('loc',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-emphasis-withdraw',cmd_witness_emphasis_withdraw,P,(('language',),{}),(('loc',),{}),(('--text',),{'required':True}),(('--reason',),{'required':True}))
    sub('witness-place-withdraw',cmd_witness_place_withdraw,P,(('language',),{}),(('loc',),{}),(('--reason',),{'required':True}))
    sub('witness-missing',cmd_witness_missing,P,(('language',),{}),(('loc',),{}),(('file',),{}));
    sub('witness-anonymous',cmd_witness_anonymous,P,(('language',),{}),(('file',),{}));
    sub('witness-collective',cmd_witness_collective,P,(('language',),{}),(('loc',),{}),(('file',),{}));
    sub('witness-collective-review',cmd_witness_collective_review,P,(('language',),{}),(('loc',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}));
    sub('witness-collective-clear',cmd_witness_collective_clear,P,(('language',),{}),(('--reason',),{'required':True}));
    sub('witness-anonymous-review',cmd_witness_anonymous_review,P,(('language',),{}),(('assessment',),{}));
    sub('witness-anonymous-clear',cmd_witness_anonymous_clear,P,(('language',),{}));
    sub('witness-unidentified-translator',cmd_witness_unidentified_translator,P,(('language',),{}),(('file',),{}));
    sub('witness-unidentified-review',cmd_witness_unidentified_review,P,(('language',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}));
    sub('witness-unidentified-clear',cmd_witness_unidentified_clear,P,(('language',),{}),(('--reason',),{'required':True}));
    sub('witness-new-translation',cmd_witness_new_translation,P,(('language',),{}),(('loc',),{}),(('--creator',),{'required':True}),(('--text',),{'required':True}),(('--image',),{'required':True}),(('--page',),{'required':True}),(('--alignment',),{'required':True}),(('--source-assessment',),{'required':True}),(('--image-assessment',),{'required':True}));
    sub('witness-new-translation-review',cmd_witness_new_translation_review,P,(('language',),{}),(('loc',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}));
    sub('witness-candidate',cmd_witness_candidate,P,(('language',),{}),(('file',),{}));
    sub('witness-person',cmd_witness_person,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('qid',),{}),(('--bounded',),{'action':'store_true'}));
    sub('witness-person-source',cmd_witness_person_source,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('name',),{}),(('url',),{}),(('--ru',),{}),(('--born',),{'type':int}),(('--died',),{'type':int}),(('--died-before',),{'type':int}),(('--bound-note',),{}),(('--anonymous',),{'action':'store_true'}),(('--file',),{}));sub('witness-person-remove',cmd_witness_person_remove,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('qid',),{}),(('--reason',),{'required':True}))
    sub('witness-reference-review',cmd_witness_reference_review,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('name',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-person-remove-legacy',cmd_witness_person_remove_legacy,P,(('language',),{}),(('name',),{}),(('--reason',),{'required':True}))
    sub('language-search-gate',cmd_language_search_gate,P,(('language',),{}),(('--second',),{'action':'store_true'}))
    sub('witness-person-remove-source',cmd_witness_person_remove_source,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('name',),{}),(('--reason',),{'required':True}))
    sub('witness-source-remove',cmd_witness_source_remove,P,(('language',),{}),(('name',),{}),(('--reason',),{'required':True}))
    sub('witness-restore-evidence',cmd_witness_restore_evidence,P,(('language',),{}),(('name',),{}),(('--file',),{}))
    sub('witness-person-publisher',cmd_witness_person_publisher,P,(('language',),{}),(('old_name',),{}),(('--full-name',),{'required':True}),(('--source',),{'required':True}),(('--isbn',),{'required':True}),(('--reason',),{'required':True}))
    sub('witness-creator',cmd_witness_creator,P,(('language',),{}),(('name',),{}));
    sub('witness-publisher-review',cmd_witness_publisher_review,P,(('language',),{}),(('name',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-person-image',cmd_witness_person_image,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('old_name',),{}),(('--name',),{'required':True}),(('--died',),{'type':int,'required':True}),(('--image',),{'required':True}),(('--page',),{'required':True}),(('--wording',),{'required':True}),(('--assessment',),{'required':True}),(('--reason',),{'required':True}))
    sub('witness-image-identity-review',cmd_witness_image_identity_review,P,(('language',),{}),(('name',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-person-bound',cmd_witness_person_bound,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('qid',),{}),(('--name',),{'required':True}),(('--source',),{'required':True}),(('--died-before',),{'type':int,'required':True}),(('--bound-note',),{'required':True}))
    sub('witness-bound-review',cmd_witness_bound_review,P,(('language',),{}),(('role',),{'choices':list(ROLES)}),(('name',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-fetch',cmd_witness_fetch,P,(('language',),{}),(('url',),{}),(('--name',),{'required':True}),(('--file',),{}))
    sub('witness-ocr',cmd_witness_ocr,P,(('language',),{}),(('image',),{}),(('--model-dir',),{'required':True}),(('--model',),{'required':True}))
    sub('witness-ocr-join',cmd_witness_ocr_join,P,(('language',),{}),(('file',),{}))
    sub('witness-ocr-join-review',cmd_witness_ocr_join_review,P,(('language',),{}),(('loc',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-image',cmd_witness_image,P,(('language',),{}),(('url',),{}),(('--name',),{'required':True}),(('--file',),{}),(('--by',),{'choices':['maker','second'],'default':'maker'}))
    sub('witness-cut',cmd_witness_cut,P,(('language',),{}),(('source',),{}),(('open',),{}),(('close',),{}),(('--loc',),{'required':True}),(('--page',),{}),(('--address-kind',),{'choices':ADDRESS_KINDS}),(('--address-value',),{}),(('--image',),{'required':True}),(('--alignment',),{'required':True}),(('--lines',),{}),(('--justification',),{}))
    sub('witness-address-correct',cmd_witness_address_correct,P,(('language',),{}),(('loc',),{}),(('--page',),{'required':True}),(('--section',),{}),(('--line-start',),{'type':int,'required':True}),(('--line-end',),{'type':int,'required':True}),(('--reason',),{'required':True}))
    sub('witness-page-identity',cmd_witness_page_identity,P,(('language',),{}),(('loc',),{}),(('--page',),{'required':True}),(('--image',),{'required':True}),(('--reader',),{'required':True}),(('--reader-url',),{'required':True}),(('--method',),{'required':True}),(('--assessment',),{'required':True}))
    sub('witness-transcribe-page',cmd_witness_transcribe_page,P,(('language',),{}),(('--text',),{'required':True}),(('--reason',),{'required':True}),(('--loc',),{'required':True}),(('--page',),{}),(('--address-kind',),{'choices':ADDRESS_KINDS}),(('--address-value',),{}),(('--image',),{'required':True}),(('--alignment',),{'required':True}),(('--lines',),{}),(('--justification',),{}))
    sub('witness-cut-candidate',cmd_witness_cut_candidate,P,(('language',),{}),(('source',),{}),(('--loc',),{'required':True}),(('--page',),{}),(('--address-kind',),{'choices':ADDRESS_KINDS}),(('--address-value',),{}),(('--image',),{'required':True}),(('--alignment',),{'required':True}),(('--lines',),{}),(('--justification',),{}));
    sub('witness-add-page',cmd_witness_add_page,P,(('language',),{}),(('loc',),{}),(('--page',),{'required':True}),(('--image',),{'required':True}),(('--span',),{'required':True}),(('--first-span',),{}));
    sub('witness-correct',cmd_witness_correct,P,(('language',),{}),(('loc',),{}),(('from',),{}),(('to',),{}),(('reason',),{}),(('--image',),{}),(('--page',),{}));
    sub('witness-withdraw',cmd_witness_withdraw,P,(('language',),{}),(('--reader',),{'required':True}),(('--finding',),{'required':True}),(('--replace',),{'action':'store_true'}))
    sub('witness-retire',cmd_witness_retire,P,(('language',),{}),(('--maker',),{'required':True}),(('--reason',),{'required':True}))
    sub('witness-review',cmd_witness_review,P,(('language',),{}),(('loc',),{}),(('--reader',),{'required':True}),(('--alignment',),{'required':True}),(('--model',),{'default':'gpt-6.1-terra'}))
    sub('page-number-correct',cmd_page_number_correct,P,(('--loc',),{'action':'append','required':True}),(('--leaf',),{'type':int,'required':True}),(('--old',),{'required':True}),(('--new',),{'required':True}),(('--url',),{'required':True}),(('--by',),{'required':True}),(('--second',),{'required':True}),(('--second-model',),{'required':True}),(('--assessment',),{'required':True}),(('--file',),{}))
    sub('accepted-text-correct',cmd_accepted_text_correct,P,(('loc',),{}),(('from',),{}),(('to',),{}),(('--n',),{'type':int,'required':True}),(('--url',),{'required':True}),(('--by',),{'required':True}),(('--second',),{'required':True}),(('--second-model',),{'required':True}),(('--assessment',),{'required':True}),(('--file',),{}))
    sub('accepted-text-index',cmd_accepted_text_index,P,(('loc',),{}),(('to',),{}))
    sub('witness-gate',cmd_witness_gate,P,(('language',),{}),(('--second',),{'action':'store_true'}),(('--offline',),{'action':'store_true'}),(('--accept',),{'action':'store_true'}))
    sub('queue',cmd_queue);sub('open',cmd_open,(('work',),{}));sub('task',cmd_task,P);sub('status',cmd_status);sub('ready',cmd_ready)
    sub('fetch',cmd_fetch,P,(('role',),{'choices':['transcription','other']}),(('url',),{}),(('--name',),{}),(('--file',),{'help':'take the bytes from a local file already fetched from URL'}))
    sub('pdf-page',cmd_pdf_page,P,(('source',),{}),(('page',),{'type':int}),(('--language',),{}),(('--loc',),{}),(('--by',),{'choices':['maker','second'],'default':'maker'}))
    sub('pdf-text',cmd_pdf_text,P,(('source',),{}),(('--language',),{'help':'derive from a translation witness source'}))
    sub('ia',cmd_ia,P,(('item',),{}),(('--long-s',),{'action':'store_true','help':'the print sets the long s, which the OCR reads as f'}),(('--dir',),{}),(('--files',),{}))
    sub('find',cmd_find,P,(('words',),{}),(('--in',),{'dest':'inside'}));sub('leaf',cmd_leaf,P,(('n',),{'type':int}));sub('scan',cmd_scan,P,(('item',),{'nargs':'?'}));sub('scan-withdraw',cmd_scan_withdraw,P,(('item',),{}),(('--reason',),{'required':True}))
    sub('image',cmd_image,P,(('n',),{'type':int}),(('--loc',),{}),(('--width',),{'type':int,'default':1100}),(('--by',),{'choices':['maker','second'],'default':'maker'}),(('--file',),{}),(('--scan',),{'help':'the Archive item, where the packet reads several scans'}),(('--zoom',),{'help':'TOP-BOTTOM in percent of the page height: the lines of a correction, enlarged'}))
    sub('offset',cmd_offset,P,(('k',),{'type':int}),(('--scan',),{}))
    sub('person',cmd_person,P,(('role',),{'choices':list(ROLES)}),(('qid',),{'nargs':'?'}),(('--name',),{}),(('--ru',),{}),(('--died',),{'type':int}),(('--source',),{}),(('--wartime',),{'choices':['yes','no']}),(('--wartime-source',),{}),(('--rehabilitated',),{'type':int}),(('--rehabilitation-source',),{}),(('--remove',),{'action':'store_true','help':'take this hand out of the record again (another edition was chosen)'}),(('--bounded',),{'action':'store_true','help':'an ancient or medieval hand: the death is bounded by the Wikidata claims, the year is not invented'}),(('--died-before',),{'type':int,'help':'the upper bound of the death a trusted reference gives (with --name, --source, --bound-note)'}),(('--bound-note',),{}),(('--anonymous',),{'action':'store_true','help':'the author is unknown; --name is the name the work goes under'}));sub('printed',cmd_printed,P,(('year',),{'type':int}))
    sub('person-bound-review',cmd_person_bound_review,P,(('role',),{'choices':list(ROLES)}),(('qid',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('rights',cmd_rights,P,(('--jurisdiction',),{'choices':JURISDICTIONS}),(('--basis',),{'choices':sorted(BASES)}),(('--law',),{}));sub('why',cmd_why,P,(('file',),{}),(('reason',),{}));sub('stamp',cmd_stamp,P,(('role',),{'choices':['maker','second']}),(('model',),{}),(('effort',),{}))
    sub('cut',cmd_cut,P,(('file',),{}),(('open',),{}),(('close',),{}),(('--loc',),{'required':True}),(('--lang',),{}),(('--nth',),{'type':int}),(('--drop',),{'action':'append','help':'a regular expression of what the transcriber added inside the run (a figure mark, a page mark); may be given more than once'}))
    sub('address',cmd_address,P,(('loc',),{}),(('kind',),{'choices':ADDRESS_KINDS}),(('value',),{}),(('url',),{}));
    sub('locator',cmd_locator,P,(('loc',),{}),(('url',),{}),(('--ru',),{'required':True}),(('--en',),{'required':True}),(('--reference',),{'required':True}),(('--searches',),{'required':True}),(('--maker-read',),{'required':True}));
    sub('transcribe-original-page',cmd_transcribe_original_page,P,(('loc',),{}),(('--url',),{'required':True}),(('--n',),{'type':int,'required':True}),(('--text',),{'required':True}),(('--reader',),{'required':True}),(('--label-ru',),{'required':True}),(('--label-en',),{'required':True}),(('--why',),{'required':True}),(('--punctuation',),{'required':True}),(('--file',),{}))
    sub('original-page-review',cmd_original_page_review,P,(('loc',),{}),(('--reader',),{'required':True}),(('--assessment',),{'required':True}))
    sub('page',cmd_page,P,(('loc',),{}),(('n',),{'type':int}),(('url',),{}),(('--file',),{}),(('--by',),{'choices':['maker','second'],'default':'maker'}));
    sub('page-withdraw',cmd_page_withdraw,P,(('loc',),{}),(('file',),{}),(('--replacement-page',),{'type':int}),(('--reason',),{'required':True}))
    sub('quotation',cmd_quotation,P,(('loc',),{}),(('source',),{}),(('page',),{}),(('lines',),{}),(('sha256',),{}),(('--justification',),{}));
    sub('compare',cmd_compare,P);sub('judge',cmd_judge,P,(('loc',),{}),(('which',),{}),(('stands',),{'choices':['transcription','print']}),(('note',),{'nargs':'?'}))
    sub('correct',cmd_correct,P,(('loc',),{}),(('from',),{}),(('to',),{}),(('evidence',),{}))
    sub('gate',cmd_gate,P,(('--second',),{'action':'store_true'}),(('--offline',),{'action':'store_true'}))
    sub('reopen-response',cmd_reopen_response,P,(('index',),{'type':int}),(('--maker',),{'required':True}),(('--assessment',),{'required':True}))
    sub('reopen-review',cmd_reopen_review,P,(('index',),{'type':int}),(('--reader',),{'required':True}),(('--resolution',),{'choices':['resolved','unresolved'],'required':True}),(('--assessment',),{'required':True}))
    sub('settle',cmd_settle,P);sub('accept',cmd_accept,P,(('--by',),{'required':True}),(('--basis',),{}),(('--offline',),{'action':'store_true'}));sub('reopen',cmd_reopen,P,(('--reader',),{'required':True}),(('--finding',),{'required':True}))
    args=p.parse_args();args.fn(args)

if __name__=='__main__':main()
