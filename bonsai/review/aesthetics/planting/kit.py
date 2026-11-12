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
import argparse,bisect,datetime,difflib,gzip,hashlib,html.parser,json,math,os,re,subprocess,sys,time,unicodedata,urllib.parse,urllib.request

HERE=os.path.dirname(os.path.abspath(__file__));REVIEW=os.path.dirname(HERE);ROOT=os.path.abspath(os.path.join(REVIEW,'..','..','..'))
PACKETS=os.path.join(HERE,'packets');HOME=os.path.join(ROOT,'bonsai','aesthetics')
CACHE=os.environ.get('TOS_PLANTING_CACHE') or os.path.join(os.path.expanduser('~'),'.cache','tos-planting')   # fetched files stay outside the checkout
FIRST_WAVE={'died':1942,'published':1929}          # every hand in the text dead by 1942, the edition printed by 1929
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
            'concepts':{concept['key']:concept for concept in concepts},'canon':canon,'titles':titles,'readings':layer('readings'),'links':layer('links'),
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
    known=w['works'][work];year=known.get('year')
    if work in w['fragments']['works']:return 'first'
    if any(record['work']==work and not record['formula']['cite'][0].get('loc') for record in w['field']['conceptions']):return 'corpus'
    if year is None:return 'undated'
    if year>FIRST_WAVE['published']:return 'later'
    died=[int(m.group(1)) for agent in known['agents'] for date in (w['agents'][agent].get('dates') or [])[:1] for m in [re.search(r'[–-]\s*(\d{4})\s*$',date)] if m]
    return 'later' if any(year>FIRST_WAVE['died'] for year in died) else 'first'

def cmd_queue(args):
    w=world();rows=[];hold=load(os.path.join(HERE,'hold.json'))
    for work in w['works']:
        t=task(w,work)
        if not t['readings']:continue
        unread=all(r['basis']=='bibliography' for r in t['readings'])      # a catalogue stands behind every waiting reading: the work itself is still to be read
        rows.append({'work':work,'wave':'held' if work in hold else 'unread' if unread else wave(w,work),'year':t['year'],'readings':len(t['readings']),'primary':sum(r['basis']=='primary' for r in t['readings']),
                     'links':len(t['links']),'areas':sorted({area for r in t['readings'] for area in r['areas']})})
    first=lambda row:sum(area in('AE01','AE02') for area in row['areas'])
    rows.sort(key=lambda row:(row['wave']!='first',-first(row),-row['readings'],row['work']))
    save(os.path.join(HERE,'queue.json'),{'made':'kit.py queue','rule':f"first wave: printed by {FIRST_WAVE['published']} and, where the layer dates the author, dead by {FIRST_WAVE['died']}; the gate decides on the dates a maker finds",'works':rows})
    count={}
    for row in rows:count.setdefault(row['wave'],[0,0]);count[row['wave']][0]+=1;count[row['wave']][1]+=row['readings']
    print('queue.json: '+'; '.join(f'{name}: {works} works, {readings} readings' for name,(works,readings) in count.items()))

# ---- a packet ----------------------------------------------------------------------------------------------------

def packet(arg):
    path=arg if os.path.isdir(arg) else os.path.join(PACKETS,arg)
    if not os.path.isdir(path):die(f'no packet at {path}')
    return os.path.abspath(path)
def work_of(path):return os.path.basename(path.rstrip('/'))
def cache(path,*names):
    home=os.path.join(CACHE,work_of(path));os.makedirs(home,exist_ok=True);return os.path.join(home,*names)
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
    path=packet(args.packet);t=task(world(),work_of(path));save(cache(path,'task.json'),t);show(t)

# ---- sources -----------------------------------------------------------------------------------------------------

def get(url,tries=3):
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':AGENT}),timeout=90) as response:return response.read()
        except Exception as error:
            if attempt==tries-1:die(f'could not fetch {url}: {error}')
            time.sleep(3*(attempt+1))

class Plain(html.parser.HTMLParser):
    """The running text of an HTML transcription: page marks, footnote anchors and notes left out, blocks as paragraphs."""
    SKIP=re.compile(r'pagenum|pageno|fnanchor|footnote|linenum|noprint|reference|mw-editsection|toc\b');BLOCK={'p','div','h1','h2','h3','h4','h5','h6','li','blockquote','tr','td','pre','table','section','hr','dd','dt'}
    def __init__(self):super().__init__();self.out=[];self.stack=[]
    def handle_starttag(self,tag,attrs):
        if tag in('br','img','hr','meta','link','input'):
            if tag=='br' and not any(self.stack):self.out.append('\n')
            if tag=='hr' and not any(self.stack):self.out.append('\n\n')
            return
        skip=tag in('script','style','head','sup','title') or bool(self.SKIP.search(dict(attrs).get('class') or ''))
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

def plain(raw,name):
    text=raw.decode('utf-8',errors='replace')
    if re.search(r'\.x?html?$',name) or re.match(r'\s*<(!doctype|html)',text[:300],re.I):
        parser=Plain();parser.feed(text);text=''.join(parser.out)
    text=unicodedata.normalize('NFC',text.replace('\r\n','\n').replace('\r','\n').replace('­','').replace('﻿',''))
    return re.sub(r'\n[ \t]*\n\s*','\n\n',re.sub(r'[ \t   ]+',' ',text))

def record(path,role,url,name,raw,**more):
    with open(cache(path,name),'wb') as file:file.write(raw)
    ev=evidence(path);ev['sources']=[source for source in ev['sources'] if source['file']!=name]
    ev['sources'].append({'role':role,'url':url,'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':len(raw),'fetched':today(),**more})
    save(os.path.join(path,'evidence.json'),ev);return ev

def cmd_fetch(args):
    path=packet(args.packet);name=args.name or re.sub(r'[^A-Za-z0-9._-]+','-',urllib.parse.urlparse(args.url).path.strip('/').split('/')[-1] or 'source')
    raw=open(args.file,'rb').read() if args.file else get(args.url)
    if name.endswith('.gz'):raw=gzip.decompress(raw);name=name[:-3]
    more={}
    if args.role=='transcription':
        text=plain(raw,name);
        with open(cache(path,name+'.txt'),'w',encoding='utf-8') as file:file.write(text)
        more={'text':name+'.txt'}
    record(path,args.role,args.url,name,raw,**more)
    print(f"{name}: {len(raw)} bytes"+(f"; running text in {cache(path,name+'.txt')} ({len(text)} characters)" if more else ''))

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
    return re.sub(r'[\W\d_]','',''.join(c for c in unicodedata.normalize('NFD',text) if not unicodedata.combining(c)).lower())

def changed(a,b):
    """The words a correction changes in their letters, as pairs (before, after)."""
    wa,wb=re.findall(r'\w+',a),re.findall(r'\w+',b);na,nb=[letters(w) for w in wa],[letters(w) for w in wb]
    return [(' '.join(wa[i1:i2]),' '.join(wb[j1:j2])) for tag,i1,i2,j1,j2 in difflib.SequenceMatcher(None,na,nb,autojunk=False).get_opcodes() if tag!='equal' and ''.join(na[i1:i2])!=''.join(nb[j1:j2])]

def by_eye(places,ev):
    """The corrections that rest on a reader's eye alone: they change letters, and the page's OCR does not read them either
    (the place was read from the image, the OCR reads otherwise there, or the print sets the long s where the change is between s and f)."""
    found=[];work=places.get('work','')
    for correction in places.get('corrections') or []:
        pairs=changed(str(correction.get('from','')),str(correction.get('to','')))
        loc=str(correction.get('place','')).removeprefix(work+' ');entry=(ev.get('places') or {}).get(loc)
        if not pairs or entry is None:continue
        read={letters(word) for difference in entry.get('differences',[]) for word in re.findall(r'\w+',difference.get('transcription',''))}
        long_s=scan_of(ev,entry).get('long_s')
        if entry.get('method')!='ocr' or any(letters(word) in read for _,after in pairs for word in re.findall(r'\w+',after)) or (long_s and any(letters(a).replace('f','s')==letters(b).replace('f','s') for a,b in pairs)):
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
        files=[entry['name'] for entry in json.loads(get(f'https://archive.org/metadata/{item}')).get('files',[])]
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

# ---- words ---------------------------------------------------------------------------------------------------------

QUOTES=str.maketrans({'“':'"','”':'"','„':'"','«':'"','»':'"','‟':'"','‘':"'",'’':"'",'‚':"'",'‛':"'",'—':'-','–':'-','‒':'-','‐':'-','‑':'-','⸗':'-','…':'.'})
TOKEN=re.compile(r"[^\W\d_]+(?:'[^\W\d_]+)*|\d+|[^\s\w]|_")
WORD=re.compile(r'[^\W\d_]+')
def key(token,long_s=False):
    token=unicodedata.normalize('NFKC',token).casefold().replace("'",'')
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
        if len(word)<4 or not places or len(places)>60:continue
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

def against_ocr(text,o):
    """A place beside the OCR: its leaves, how much of it the OCR reads the same, and every difference."""
    mine=tokens(text);a=[token for token,_ in mine];ak=[key(token,o['long_s']) for token in a]
    if 'keys' not in o:
        o['keys']=[key(token,o['long_s']) for token in o['words']];o['index']={}
        for position,word in enumerate(o['keys']):o['index'].setdefault(word,[]).append(position)
    best=locate(ak,o['keys'],o['index'])
    if not best:return None
    matched,start,match=best;blocks=[block for block in match.get_matching_blocks() if block.size]
    if not blocks or matched/len(ak)<0.75:return {'share':round(matched/len(ak),3) if ak else 0}
    leaf=lambda position:bisect.bisect_right(o['starts'],o['at'][min(start+position,len(o['at'])-1)])-1
    ops=match.get_opcodes();differences=[]
    for n,(tag,i1,i2,j1,j2) in enumerate(ops):
        if tag=='equal' or (tag=='insert' and n in(0,len(ops)-1)):continue
        differences.append({'transcription':' '.join(a[i1:i2]),'ocr':' '.join(o['words'][start+j1:start+j2]),'leaf':leaf(j1)})
    first,last=leaf(blocks[0].b),leaf(blocks[-1].b+blocks[-1].size-1)
    return {'share':round(matched/len(ak),3),'leaves':list(range(first,last+1)),'printed':[o['printed'].get(n) for n in range(first,last+1)],'differences':differences}

def in_transcription(text,source,dropped=None):
    """Whether a place is a run of the transcription, letter for letter as kit.py cut writes it, and opens a sentence there."""
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
    opened=bool(re.search(r'\n\s*\n[\s"“„«‘\'(\[_*]*$',before)) or not before.strip()
    before=re.sub(r'(\[[^\]]{0,12}\]|\(\d+\)|[\d*†‡§]+)\s*$','',re.sub(r'[\s"“„«‘\'(\[_*]+$','',before))
    before=re.sub(r'[\s"”»’\')\]_*]+$','',re.sub(r'[\s—–-]+$','',re.sub(r'[\s"”»’\')\]_*]+$','',before)))   # «… kommen. — Eben jene …»: the dash after a full stop opens a sentence
    if not opened and before and before[-1] not in '.!?:…':return f'it opens inside a sentence of the transcription (after «…{before[-40:]}»)'
    blocks=[block for block in match.get_matching_blocks() if block.size];a=words[start+blocks[0].b][1];last=words[start+blocks[-1].b+blocks[-1].size-1];z=last[1]+len(last[0])
    def written(end):
        run=source[a:end]
        for pattern in dropped or []:run=re.sub(pattern,' ',run)
        return re.sub(r' +([,;:.!?])',r'\1',squeeze(run)) if dropped else squeeze(run)
    if any(written(z+more)==text for more in range(0,60)):return None
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
    return sources[0],open(cache(path,sources[0]['text']),encoding='utf-8').read()

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
    run=re.sub(r' +([,;:.!?])',r'\1',squeeze(run)) if args.drop else squeeze(run);places=part(path,'places.json');original=places.get('entry',{}).get('original') or world()['fragments']['works'].get(places['work'],{}).get('original')
    lang=args.lang or original
    if not lang:die('places.json names no original language yet (entry.original)')
    place=places['places'].setdefault(args.loc,{'label':{'ru':'','en':''},'texts':{},'links':{},'refs':[]});place['texts'][lang]=run
    save(os.path.join(path,'places.json'),places);entry=ev['places'].setdefault(args.loc,{});entry['transcription']=source['file'];entry.pop('dropped',None);entry.pop('from_scan',None)
    if source['role']=='scan-ocr':entry['from_scan']=True;print('cut from the scan’s own OCR: there is no second text behind this place, so it is read whole on the page image (image N --loc, correct, judge … whole)')
    if args.drop:entry['dropped']=args.drop
    save(os.path.join(path,'evidence.json'),ev)
    print(f'{args.loc} [{lang}] {len(run)} characters:\n{run}')
    for problem in sentences(run):print('  ! '+problem)

OPENING='"“„«‚‘\'([—–- ';CLOSING='"”“»’‘\')] '        # the marks of speech and brackets a sentence may stand in: a quoted sentence is a whole sentence

def sentences(text):
    problems=[]
    if text!=unicodedata.normalize('NFC',text):problems.append('the text is not NFC')
    if text!=text.strip() or '  ' in text or '\t' in text:problems.append('stray whitespace')
    if not text.lstrip(OPENING)[:1].isupper():problems.append(f'it does not open with a capital letter («{text[:24]}»): a place runs over whole sentences')
    if text.rstrip(CLOSING)[-1:] not in '.!?…':problems.append(f'it does not close a sentence («{text[-24:]}»)')
    if len(text)>LONGEST:problems.append(f'{len(text)} characters: a place is the sentences that say the thing, at most {LONGEST} characters')
    return problems

def cmd_find(args):
    path=packet(args.packet);ev=evidence(path)
    if args.inside:
        _,text=transcription(path,ev,args.inside);flat=re.compile(r'\s+'.join(re.escape(word) for word in args.words.split()),re.I)
        for m in list(flat.finditer(text))[:12]:print(f'@{m.start()}: …{" ".join(text[max(0,m.start()-160):m.end()+260].split())}…\n')
        return
    found=False
    for item,scan in scans(ev).items():
        o=ocr(path,ev,scan);got=against_ocr(args.words,o) if o else None
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
    return open(file,'rb').read()

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
    return open(file,'rb').read()

def cmd_image(args):
    path=packet(args.packet);ev=evidence(path)
    item=args.scan or (scan_of(ev,ev['places'].get(args.loc)).get('item') if args.loc and ev['places'].get(args.loc) else None) or (ev.get('scan') or {}).get('item') or die('name the scan first (kit.py ia)')
    if item not in scans(ev):die(f'{item} is not a scan of this packet (kit.py ia)')
    if args.zoom:
        try:top,bottom=(int(part_) for part_ in args.zoom.split('-'));assert 0<=top<bottom<=100 and bottom-top<=25
        except (ValueError,AssertionError):die('--zoom wants the band of the page height in percent that holds the lines, as 41-48 (25 at most)')
        if not args.loc:die('--zoom wants --loc: the place whose line is enlarged')
        name=f'{item}-n{args.n}-z{top}-{bottom}.jpg'
        with open(cache(path,name),'wb') as file:file.write(open(args.file,'rb').read() if args.file else get(f'https://archive.org/download/{item}/page/n{args.n}.jpg'))
        raw=band(cache(path,name),top,bottom)
        ev['images']=[image for image in ev.get('images',[]) if not(image['file']==name and image.get('by')==args.by)]
        ev['images'].append({'n':args.n,'item':item,'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'by':args.by,'loc':args.loc,'zoom':[top,bottom]})
        save(os.path.join(path,'evidence.json'),ev);print(f'{cache(path,name)}\nlook at this file: the lines between {top}% and {bottom}% of the page, at the size of the scan. Where the line sits elsewhere, ask again with another band');return
    name=f'{item}-n{args.n}.jpg';url=f'https://archive.org/download/{item}/page/n{args.n}_w{args.width}.jpg'
    raw=open(args.file,'rb').read() if args.file else get(url)
    with open(cache(path,name),'wb') as file:file.write(raw)
    raw=narrow(cache(path,name),args.width)
    ev['images']=[image for image in ev.get('images',[]) if not(image['file']==name and image.get('by')==args.by)]
    ev['images'].append({'n':args.n,'item':item,'file':name,'sha256':hashlib.sha256(raw).hexdigest(),'by':args.by,**({'loc':args.loc} if args.loc else {})})
    if args.loc and args.by=='maker':
        entry=ev['places'].setdefault(args.loc,{})
        if entry.get('method')!='ocr':entry['method']='image';entry['images']=sorted(set(entry.get('images',[]))|{args.n});entry.setdefault('scan',item)
    save(os.path.join(path,'evidence.json'),ev);print(f'{cache(path,name)}\nlook at this file; the page of the place opens at https://archive.org/details/{item}/page/n{args.n}/mode/1up')

TRUSTED=('wikipedia.org','wikidata.org','d-nb.info','viaf.org','bnf.fr','plato.stanford.edu','britannica.com','deutsche-biographie.de','loc.gov','oxforddnb.com','treccani.it')
def trusted(url):
    host=urllib.parse.urlparse(str(url)).netloc.lower();return str(url).startswith('https://') and any(host==name or host.endswith('.'+name) for name in TRUSTED)

def cmd_person(args):
    """A hand in the text, by its Wikidata item: the kit reads the name and the year of death there, so that no date is typed."""
    path=packet(args.packet);ev=evidence(path)
    if args.remove:
        kept=[other for other in ev['persons'] if not(other.get('role')==args.role and (other.get('qid')==args.qid if args.qid else (other.get('name') or {}).get('en')==args.name if isinstance(other.get('name'),dict) else other.get('name')==args.name))]
        if len(kept)==len(ev['persons']):die('no such person is on record: '+'; '.join(f"{other.get('role')} {other.get('qid') or ''} {other.get('name')}" for other in ev['persons']))
        ev['persons']=kept;save(os.path.join(path,'evidence.json'),ev);write_rights(path,ev);print(f'{len(kept)} persons stay on record');return
    if args.qid:
        if not re.fullmatch(r'Q\d+',args.qid):die('a Wikidata item reads Q12345')
        entity=json.loads(get(f'https://www.wikidata.org/wiki/Special:EntityData/{args.qid}.json'))['entities'][args.qid]
        label=lambda lang:(entity.get('labels',{}).get(lang) or {}).get('value')
        year=lambda prop:next((int(claim['mainsnak']['datavalue']['value']['time'][:5]) for claim in sorted(entity.get('claims',{}).get(prop,[]),key=lambda claim:claim.get('rank')!='preferred') if claim['mainsnak'].get('datavalue')),None)
        died=year('P570');print(f"{args.qid}: {label('en')} / {label('ru')} — {(entity.get('descriptions',{}).get('en') or {}).get('value')}; born {year('P569')}, died {died}")
        if died is None:die('Wikidata gives no date of death here: give --name, --died and --source (a reference page) instead of an item')
        person={'qid':args.qid,'name':{'en':label('en'),'ru':label('ru') or label('en')},'role':args.role,'died':died,'source':f'https://www.wikidata.org/wiki/{args.qid}'}
    else:
        if not(args.name and args.died is not None and trusted(args.source)):die('without an item: --name NAME --died YEAR --source URL, the URL on one of: '+', '.join(TRUSTED))
        person={'name':{'en':args.name,'ru':args.ru or args.name},'role':args.role,'died':args.died,'source':args.source}
    ev['persons']=[other for other in ev['persons'] if isinstance(other.get('name'),dict) and not(other.get('role')==args.role and other['name']==person['name'])]+[person]   # a record of an older shape is entered again, not kept
    save(os.path.join(path,'evidence.json'),ev);write_rights(path,ev);print(f"{person['name']['en']} ({args.role}) died {person['died']}")

ROLES={'author':('автор','author'),'editor':('редактор текста','editor of the text'),'translator':('переводчик','translator')}
def rights_lines(ev):
    """The three rights sentences of a first-wave witness, written from the dates alone."""
    people=sorted(ev.get('persons') or [],key=lambda person:list(ROLES).index(person['role']));when=lambda year,ru:(f'{year}' if year>0 else f'{-year} до н. э.' if ru else f'{-year} BCE')
    return {'russia':'Общественное достояние: '+'; '.join(f"{person['name']['ru']} ({ROLES[person['role']][0]}) — год смерти {when(person['died'],True)}" for person in people)+' — ГК РФ ст. 1281 (70 лет p.m.a.) с ФЗ № 231-ФЗ от 18.12.2006, ст. 6: 50-летний срок истёк до 1 января 1993 г. (смерть до 1943 г.), произведение в общественном достоянии.',
            'usa':f"Public domain: this printing was published in {ev.get('published')} (before 1930).",
            'eu':'Public domain: '+'; '.join(f"{person['name']['en']} ({ROLES[person['role']][1]}) died {when(person['died'],False)}" for person in people)+', more than 70 years ago (Directive 2006/116/EC art. 1).'}
def write_rights(path,ev):
    places=part(path,'places.json');entry=(places or {}).get('entry') or {};witness=(entry.get('witnesses') or {}).get(entry.get('original'))
    if witness is None or not ev.get('persons') or not isinstance(ev.get('published'),int):return
    witness['rights']={'basis':'public-domain',**rights_lines(ev),'reasoning':(witness.get('rights') or {}).get('reasoning','')};save(os.path.join(path,'places.json'),places)

def cmd_rights(args):
    path=packet(args.packet);ev=evidence(path);write_rights(path,ev);print(json.dumps(rights_lines(ev),ensure_ascii=False,indent=1))

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
        text=place.get('texts',{}).get(original);old=ev['places'].get(loc,{});entry={key_:old[key_] for key_ in('transcription','dropped','from_scan','scan','whole','images') if key_ in old}
        if not text:out[loc]=entry;continue
        fixed=scan_of(ev,old).get('item') if old.get('scan') or old.get('from_scan') else None      # a place stays with its scan; a new one is looked for in each
        got=None
        for item in [fixed] if fixed in every else [active]+[item for item in every if item!=active]:
            o=reading(item) if item in every else None;found=against_ocr(text,o) if o else None
            if found and (got is None or 'leaves' in found or (found.get('share') or 0)>(got.get('share') or 0)):got=found;entry['scan']=item
            if found and 'leaves' in found:break
        if entry.get('from_scan') and got and 'leaves' in got:               # the OCR beside itself proves nothing: the page is read instead
            entry.update({'method':'image','leaves':got['leaves'],'printed':got['printed']});out[loc]=entry;continue
        if not got or 'leaves' not in got:
            entry.update({'method':'image','share':(got or {}).get('share')});out[loc]=entry;continue
        judged={(d['transcription'],d['ocr'],d['leaf']):d for d in old.get('differences',[])}
        for d in got['differences']:
            was=judged.get((d['transcription'],d['ocr'],d['leaf']))
            if was:d.update({key_:was[key_] for key_ in('stands','note') if key_ in was})
        entry.update({'method':'ocr','share':got['share'],'leaves':got['leaves'],'printed':got['printed'],'differences':got['differences']});out[loc]=entry
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

# ---- the gate ------------------------------------------------------------------------------------------------------

NEGATES={'ru':r'\b(не|ни|нет|без|безо|никогда|никак|ничто|ничего|никто|никакой|никакая|никакое|никакие|нельзя|невозможно)\b',
         'en':r"\b(not|no|never|without|neither|nor|none|nothing|cannot|nobody|nowhere)\b|n['’]t\b|\b\w{3,}less\b",
         'de':r'\b(nicht|nichts|kein\w*|ohne|nie|niemals|nimmer|weder|niemand|nirgends)\b|\b\w{3,}los(e[nrsm]?)?\b','fr':r"\b(ne|n|pas|point|sans|ni|jamais|rien|aucune?|nul\w*|non)\b",
         'it':r'\b(non|senza|né|mai|nulla|niente|nessun\w*)\b','la':r'\b(non|ne|nec|neque|sine|nihil|haud|null\w*|nemo|numquam|nunquam|minime)\b','es':r'\b(no|sin|ni|nunca|nada|ningun\w*|jamás)\b'}
CONTRAST={'ru':r'\bне\s+(только|просто|столько|лишь)\b|\bа\s+не\b|\bвместо\b|\bнежели\b','en':r'\bnot\s+(only|merely|simply|just)\b|\brather\s+than\b|\binstead\s+of\b'}
HEDGES={'de':r'\b(vielleicht|gleichsam|gewissermaßen|eine Art|scheint|scheinen)\b','fr':r'\b(peut-être|en quelque sorte|une (sorte|espèce) de|semblent?)\b','it':r'\b(forse|una (sorta|specie) di|sembra|quasi)\b','la':r'\b(quasi|fortasse|forsitan|videtur|quodammodo)\b',
        'ru':r'\b(возможно|вероятно|по-видимому|видимо|пожалуй|как бы|в (некотором|каком-то|известном) смысле|можно сказать|так сказать|(следует|важно|стоит|нужно|необходимо) (отметить|подчеркнуть)|на самом деле|своего рода)\b',
        'en':r'\b(perhaps|arguably|maybe|seems? to|appears? to|in a sense|so to speak|it is worth noting|it is important to note|kind of|sort of)\b'}

GROUNDS={'rights':'a hand in the text died after 1942, or no printing from 1929 or earlier is scanned','sources':'no scan of the work, or no text to cut the words from','reading':'the work is not the one the registers read','kit':'the kit itself fails'}

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

def gate(path,second=False,offline=False):
    problems=[];work=work_of(path);add=problems.append
    owe=OWES.append if part(path,'accepted.json') is not None else add
    blocked=part(path,'blocked.json')
    if blocked is not None:
        if len(str(blocked.get('reason','')))<40 or not blocked.get('tried'):add('blocked.json: give the reason in a full sentence and list under tried what was looked at (with URLs)')
        if blocked.get('ground') not in GROUNDS:add('blocked.json: ground wants one of '+'; '.join(f'{name} ({what})' for name,what in GROUNDS.items()))
        if blocked.get('ground')=='sources' and any(scan.get('ocr') for scan in scans(part(path,'evidence.json') or {}).values()):
            add('blocked.json: the packet holds a scan with page-indexed OCR, so the words can be cut from the page itself (kit.py cut . ocr …, MAKER.md step 3). A project whose site did not answer is named in kit.py why; it does not close this road. Take blocked.json away and make the packet')
        if blocked.get('ground')=='rights':
            ev=part(path,'evidence.json') or {};people=[person for person in ev.get('persons') or [] if isinstance(person.get('died'),int)];printed=ev.get('published')
            if not any(person['died']>FIRST_WAVE['died'] for person in people) and not(isinstance(printed,int) and printed>FIRST_WAVE['published']):
                add(f"blocked.json: the dates on record are inside the first wave ({'; '.join(str((person.get('name') or {}).get('en') if isinstance(person.get('name'),dict) else person.get('name'))+' died '+str(person['died']) for person in people) or 'no person entered'}; printing {printed}). «Dead by {FIRST_WAVE['died']}» means in {FIRST_WAVE['died']} or any year before it, «printed by {FIRST_WAVE['published']}» likewise. Take blocked.json away and make the packet")
        return problems,'blocked' if not problems else 'shut'
    places,reading,ev=part(path,'places.json'),part(path,'reading.json'),part(path,'evidence.json')
    for name,value in(('places.json',places),('reading.json',reading),('evidence.json',ev)):
        if value is None:add(f'{name} is missing')
    if problems:return problems,'shut'
    w=world(but=(reading.get('reader') or {}).get('id'))
    if work not in w['works']:return [f'{work} is not a work of the layer'],'shut'
    entry=places.get('entry');t=task(w,work)
    held=None if entry else w['fragments']['works'].get(work)        # a packet with an entry brings the whole work; one without adds places to a work the bonsai holds
    if places.get('work')!=work:add('places.json names another work')
    if entry and set(w['fragments']['places'].get(work,{}))-set(places.get('places') or {}):add('the bonsai already holds places of this work: drop entry from places.json and add places of the same edition')
    if not held and not entry:add('places.json has no entry for the work')
    entry=entry or held or {};original=entry.get('original') or ''
    # the witness and the rights
    if not held:
        for name in('author','title'):
            if not all((entry.get(name) or {}).get(lang) for lang in('ru','en')):add(f'entry.{name} wants ru and en')
        if not re.fullmatch(r'[a-z]{2,3}',original):add('entry.original wants a language code (de, en, fr, it, la, ru, el …)')
        witnesses=entry.get('witnesses') or {}
        if list(witnesses)!=[original]:add('entry.witnesses holds one witness in this pass: the original, under its language code')
        witness=witnesses.get(original) or {}
        for name in('edition','licence','attribution'):
            if not isinstance(witness.get(name),str) or len(witness.get(name,''))<20:add(f'witness.{name} is missing or too short to name the book')
        if not isinstance(witness.get('year'),int):add('witness.year wants the year of the printing as a number')
        if not str(witness.get('url','')).startswith('https://'):add('witness.url wants the https address of the scan')
        from_scan=bool(ev.get('places')) and all(entry.get('from_scan') for entry in ev['places'].values())
        if len([s for s in witness.get('sources') or [] if str(s).startswith('https://')])<(1 if from_scan else 2):add('witness.sources wants the scan and the transcription (two https addresses; the scan alone where every place is cut from its OCR)')
        rights=witness.get('rights') or {}
        if rights.get('basis')!='public-domain':add('witness.rights.basis must be public-domain in the first wave')
        if len(str(rights.get('reasoning','')))<60:add('witness.rights.reasoning wants its sentences: whose text, which transcription (and which edition it follows), which scan, what was normalised')
        gone={item.get('language') for item in entry.get('absent') or [] if len(str(item.get('reason','')))>=20}
        for lang in('ru','en'):
            if lang!=original and lang not in gone:add(f'entry.absent wants {{language: {lang}, reason}}: no {lang} witness is taken in this pass')
        persons=ev.get('persons') or []
        if not any(person.get('role')=='author' for person in persons):add('evidence.persons wants the author with the year of death and the page that gives it')
        sound=True
        for person in persons:
            if not isinstance(person.get('died'),int) or not isinstance(person.get('name'),dict) or not(person.get('qid') or trusted(person.get('source'))):sound=False;add(f"evidence.persons {person.get('name')}: enter the person again with kit.py person ROLE QID (the Wikidata item gives the name and the year of death)")
            elif person['died']>FIRST_WAVE['died']:add(f"{person['name']['en']} died in {person['died']}: outside the first wave (by {FIRST_WAVE['died']}). If this hand belongs to an edition you are not reading, take it out with kit.py person ROLE QID --remove and read an edition whose hands all died by {FIRST_WAVE['died']}; if every scanned edition carries such a hand, write blocked.json")
        known={word for agent in w['works'][work]['agents'] for value in w['agents'][agent]['name'].values() for word in WORD.findall(value.casefold()) if len(word)>3}
        if sound and persons and known and not any(person['role']=='author' and known&set(WORD.findall(' '.join(person['name'].values()).casefold())) for person in persons):add('evidence.persons: the author entered is not the author the layer names for this work')
        if not isinstance(ev.get('published'),int):add('evidence.published wants the year of the printing used')
        elif ev['published']>FIRST_WAVE['published']:add(f"printed in {ev['published']}: outside the first wave (by {FIRST_WAVE['published']}); take an earlier printing or write blocked.json")
        elif ev['published']!=witness.get('year'):add('evidence.published and witness.year differ')
        elif sound and persons and {name:rights.get(name) for name in('russia','usa','eu')}!=rights_lines(ev):add('witness.rights.russia/usa/eu are written by the kit from the dates: run kit.py rights')
    # the places
    locs=places.get('places') or {}
    if not locs:add('places.json holds no place')
    corrections=places.get('corrections') or []
    for loc,place in locs.items():
        at=f'place {loc}';text=(place.get('texts') or {}).get(original)
        if loc!=loc.strip() or not re.search(r'\S p\d+(-\d+)?[a-z]?$',loc):add(f'{at}: a place is named by its division and the printed page it opens on, as «II p16a», «Letter XV p164», «§28 p112»')
        for lang in('ru','en'):
            if re.search(r'(pp\.|сс\.)\s*\d+\s*$',str((place.get('label') or {}).get(lang,''))) and not re.search(r'\d\s*[–-]\s*\d+\s*$',str(place['label'][lang])):add(f'{at}: label.{lang} names one page: «p. 226», «с. 226»')
        if held and loc in w['fragments']['places'].get(work,{}) and place!=w['fragments']['places'][work][loc]:add(f'{at}: the bonsai already holds a place of this name')
        if not all((place.get('label') or {}).get(lang) for lang in('ru','en')):add(f'{at}: label wants ru and en (division and printed page)')
        if list(place.get('texts') or {})!=[original] or not isinstance(text,str):add(f'{at}: texts holds the original only, under {original}');continue
        for problem in sentences(text):add(f'{at}: {problem}')
        if not str((place.get('links') or {}).get(original,'')).startswith('https://'):add(f'{at}: links.{original} wants the address of the page of the scan')
        if not any(isinstance(ref,list) and len(ref)==2 and ref[0]==original and ref[1] for ref in place.get('refs') or []):add(f'{at}: refs wants [["{original}", "edition, printed page"]]')
    for correction in corrections:
        loc=str(correction.get('place','')).removeprefix(work+' ')
        if loc not in locs or not all(correction.get(name) for name in('lang','from','to','evidence')):add(f'correction {correction.get("place")}: lang, from, to, evidence and an existing place are wanted')
        elif correction['to'] not in locs[loc]['texts'].get(original,''):add(f'correction {correction["place"]}: «{correction["to"]}» is not in the place')
        elif correction['from']==correction['to']:add(f'correction {correction["place"]}: it changes nothing; take it out of corrections')
        elif any(scan.get('long_s') for scan in scans(ev).values()) and correction['from'].replace('ß','ss')==correction['to'].replace('ß','ss'):add(f'correction {correction["place"]}: «{correction["from"]}» → «{correction["to"]}» only trades ß and ss. In a print with the long s, ſs is the print’s ß and stays ß; ſſ is ss. Put the text back with kit.py cut and take this out of corrections')
    # a letter changed against the transcription, which the OCR does not read either, is read on the enlarged line
    for loc,what in by_eye(places,ev):
        if loc in locs and loc not in enlarged(ev,'maker'):owe(f'correction {work} {loc}: {what} rests on the eye alone, for the transcription reads otherwise and the OCR of the page does not read it either. Look at the line at the size of the scan: kit.py image . N --loc "{loc}" --zoom TOP-BOTTOM (the leaf of the place; the band of the page height in percent that holds the line). Keep the correction where the enlarged line bears it letter by letter; where it does not, put the word of the transcription back (kit.py cut) and take the correction out')
    # the evidence, recomputed from the fetched files
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
            back=text
            for correction in corrections:
                if correction.get('place')==f'{work} {loc}':back=back.replace(correction['to'],correction['from'],1)
            problem=in_transcription(back,source,mine.get('dropped'))
            if problem:add(f'{at}: {problem}')
            strip=lambda entry:[(d['transcription'],d['ocr'],d['leaf']) for d in entry.get('differences',[])]
            if now.get('method')=='ocr':
                if mine.get('method')!='ocr' or strip(mine)!=strip(now) or mine.get('leaves')!=now.get('leaves') or mine.get('scan',now.get('scan'))!=now.get('scan'):add(f'{at}: the comparison on file is not the comparison of this text (run kit.py compare)');continue
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
        run=subprocess.run(['node',os.path.join(REVIEW,'plant.mjs'),'--dry','--packet',work],capture_output=True,text=True)
        if run.returncode and thrown(run.stderr):add(LINE_FAULT.format(error=thrown(run.stderr)))
        elif run.returncode:problems.extend(f'plant: {line}' for line in (run.stdout+run.stderr).strip().splitlines()[-12:])
    if second and not problems:problems.extend(second_gate(path,reading,ev,locs,owe))
    return problems,'open' if not problems else 'shut'

def second_gate(path,reading,ev,locs,owe=None):
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
    seen=shots(ev,'second');looked=[item for item in review.get('images') or [] if item.get('loc') in locs]   # a page looked at for a place that has since left the packet asks nothing
    need=min(len(locs),max(3,math.ceil(len(locs)*0.3)))
    whole=[loc for loc,entry in ev['places'].items() if entry.get('method')=='image' and loc in locs]     # a place that has left the packet stays in the evidence and asks nothing
    if len({item.get('loc') for item in looked})<need:add(f'review.images: the second reader looks at the pages of at least {need} places; {len(looked)} are recorded')
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
    clean=all(answer.get('says')=='yes' for kind in('conceptions','links') for answer in (review.get(kind) or {}).values()) and all(item.get('reads')=='as the packet' for item in review.get('images') or []) and all((answers.get(key_) or {}).get('holds') is True for key_ in waits)
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

def cmd_accept(args):
    path=packet(args.packet);problems,state=gate(path,second=not args.basis,offline=args.offline)
    if problems:
        for problem in problems:print('- '+problem)
        die('the gate is shut: nothing is accepted')
    review=part(path,'review.json');reading=part(path,'reading.json');ev=evidence(path)
    used={entry.get('transcription') for entry in ev['places'].values()};first=next((source['file'] for source in ev['sources'] if source['role']=='transcription'),None)
    ev['sources']=[source for source in ev['sources'] if source['role']!='transcription' or source['file'] in used or (None in used and source['file']==first)];save(os.path.join(path,'evidence.json'),ev)   # a place that names no transcription was cut from the first
    basis=args.basis or f"Made by {reading['reader']['id']}; every conception and link read a second time by {review['reader']['id']}, who confirms each; the gate recomputed the comparison with the scan."
    save(os.path.join(path,'accepted.json'),{'by':args.by,'date':today(),'basis':basis,'conceptions':len(reading['conceptions']),'links':len(reading['links']),'left':len(reading.get('left',[]))})
    print(f'accepted: {work_of(path)}')

def cmd_status(args):
    rows=[]
    for name in sorted(os.listdir(PACKETS)) if os.path.isdir(PACKETS) else []:
        path=os.path.join(PACKETS,name)
        if not os.path.isdir(path):continue
        reading=part(path,'reading.json') or {};state='accepted' if part(path,'accepted.json') else 'blocked' if part(path,'blocked.json') else (load(cache(path,'gate.json')) if os.path.exists(cache(path,'gate.json')) else {}).get('state','—')
        rows.append(f"{state:9} {name}  conceptions {len(reading.get('conceptions',[]))} links {len(reading.get('links',[]))} left {len(reading.get('left',[]))} review {'yes' if part(path,'review.json') else 'no'}")
    print('\n'.join(rows) or 'no packets')

def main():
    p=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter);s=p.add_subparsers(dest='cmd',required=True)
    def sub(name,fn,*arguments):
        q=s.add_parser(name);q.set_defaults(fn=fn)
        for names,options in arguments:q.add_argument(*names,**options)
    P=(('packet',),{})
    sub('queue',cmd_queue);sub('open',cmd_open,(('work',),{}));sub('task',cmd_task,P);sub('status',cmd_status);sub('ready',cmd_ready)
    sub('fetch',cmd_fetch,P,(('role',),{'choices':['transcription','other']}),(('url',),{}),(('--name',),{}),(('--file',),{'help':'take the bytes from a local file already fetched from URL'}))
    sub('ia',cmd_ia,P,(('item',),{}),(('--long-s',),{'action':'store_true','help':'the print sets the long s, which the OCR reads as f'}),(('--dir',),{}),(('--files',),{}))
    sub('find',cmd_find,P,(('words',),{}),(('--in',),{'dest':'inside'}));sub('leaf',cmd_leaf,P,(('n',),{'type':int}));sub('scan',cmd_scan,P,(('item',),{'nargs':'?'}))
    sub('image',cmd_image,P,(('n',),{'type':int}),(('--loc',),{}),(('--width',),{'type':int,'default':1100}),(('--by',),{'choices':['maker','second'],'default':'maker'}),(('--file',),{}),(('--scan',),{'help':'the Archive item, where the packet reads several scans'}),(('--zoom',),{'help':'TOP-BOTTOM in percent of the page height: the lines of a correction, enlarged'}))
    sub('offset',cmd_offset,P,(('k',),{'type':int}),(('--scan',),{}))
    sub('person',cmd_person,P,(('role',),{'choices':list(ROLES)}),(('qid',),{'nargs':'?'}),(('--name',),{}),(('--ru',),{}),(('--died',),{'type':int}),(('--source',),{}),(('--remove',),{'action':'store_true','help':'take this hand out of the record again (another edition was chosen)'}));sub('printed',cmd_printed,P,(('year',),{'type':int}))
    sub('rights',cmd_rights,P);sub('why',cmd_why,P,(('file',),{}),(('reason',),{}));sub('stamp',cmd_stamp,P,(('role',),{'choices':['maker','second']}),(('model',),{}),(('effort',),{}))
    sub('cut',cmd_cut,P,(('file',),{}),(('open',),{}),(('close',),{}),(('--loc',),{'required':True}),(('--lang',),{}),(('--nth',),{'type':int}),(('--drop',),{'action':'append','help':'a regular expression of what the transcriber added inside the run (a figure mark, a page mark); may be given more than once'}))
    sub('compare',cmd_compare,P);sub('judge',cmd_judge,P,(('loc',),{}),(('which',),{}),(('stands',),{'choices':['transcription','print']}),(('note',),{'nargs':'?'}))
    sub('correct',cmd_correct,P,(('loc',),{}),(('from',),{}),(('to',),{}),(('evidence',),{}))
    sub('gate',cmd_gate,P,(('--second',),{'action':'store_true'}),(('--offline',),{'action':'store_true'}))
    sub('settle',cmd_settle,P);sub('accept',cmd_accept,P,(('--by',),{'required':True}),(('--basis',),{}),(('--offline',),{'action':'store_true'}))
    args=p.parse_args();args.fn(args)

if __name__=='__main__':main()
