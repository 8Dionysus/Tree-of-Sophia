#!/usr/bin/env python3
"""Runs the readers of the planting line. The operator's tool: a reader never starts it.

  drive.py run WORK                 one work from start to accepted, blocked or stuck
  drive.py line [--max 2] [--works a,b] [--count N] [--wave first]
                                    works of the queue, several at a time, until the queue, the count or the usage limit ends

One work, one fresh Codex session as the maker; the gate decides whether the maker is done, and while the gate is
shut the same session is resumed with the gate's own lines (a reader that stops early is sent back by the gate, not
by a sentence in its brief). Then a fresh session as the second reader. What the second reader does not confirm
goes back to the maker (twice at most), is read again, and whatever is still unconfirmed leaves the reading
(kit.py settle); `--retake` sends a finished packet through once more to take up what was left disputed.
Every session is `codex exec`, never plan mode and never an app-server turn.

Environment: CODEX (binary), TOS_PLANTING_MODEL, TOS_PLANTING_EFFORT, TOS_PLANTING_OUT (logs and state).
"""
import argparse,concurrent.futures,datetime,json,os,subprocess,sys,threading,time

HERE=os.path.dirname(os.path.abspath(__file__));KIT=os.path.join(HERE,'kit.py');PACKETS=os.path.join(HERE,'packets')
CODEX=os.environ.get('CODEX') or os.path.expanduser('~/.local/bin/codex')
MODEL=os.environ.get('TOS_PLANTING_MODEL','gpt-6.1-terra');EFFORT=os.environ.get('TOS_PLANTING_EFFORT','high')
OUT=os.environ.get('TOS_PLANTING_OUT') or os.path.expanduser('~/.local/share/claude-codex/out/planting')
MAKER_TURNS,SECOND_TURNS=5,3
RETURNS=2                                           # how often a packet goes back to its maker before what is unconfirmed leaves the reading
TURN=3*3600                                         # one turn of a reader, in seconds
LIMIT=threading.Event()

def now():return datetime.datetime.now().isoformat(timespec='seconds')
def out(work,*names):
    home=os.path.join(OUT,work);os.makedirs(home,exist_ok=True);return os.path.join(home,*names)
def state(work,**change):
    path=out(work,'state.json');value=json.load(open(path)) if os.path.exists(path) else {'work':work,'sessions':[],'tokens':{}}
    if change:
        value.update(change);value['at']=now()
        with open(path,'w') as file:json.dump(value,file,ensure_ascii=False,indent=1)
    return value
def kit(*args):
    run=subprocess.run([sys.executable,KIT,*args],capture_output=True,text=True);return run.returncode,(run.stdout+run.stderr).strip()

def codex(work,role,prompt,resume=None):
    """One codex exec turn in the packet directory. Returns the thread, whether the usage limit ended it, and its tokens."""
    st=state(work);n=len(st['sessions'])+1;log=out(work,f'{n:02d}-{role}.jsonl');last=out(work,f'{n:02d}-{role}.md')
    command=[CODEX,'exec']+(['resume',resume] if resume else [])+['--json','-m',MODEL,'-c',f'model_reasoning_effort={EFFORT}','--dangerously-bypass-approvals-and-sandbox','--skip-git-repo-check','-o',last,prompt]
    started=now()
    with open(log,'w') as file:
        try:code=subprocess.run(command,cwd=os.path.join(PACKETS,work),stdout=file,stderr=subprocess.STDOUT,timeout=TURN).returncode
        except subprocess.TimeoutExpired:code='timeout'
    thread=resume;limit=False;tokens={}
    for line in open(log,errors='replace'):
        try:event=json.loads(line)
        except ValueError:continue
        if event.get('type')=='thread.started':thread=event.get('thread_id')
        message=json.dumps(event.get('error') or event.get('message') or '') if event.get('type') in('error','turn.failed') else ''
        if 'usage limit' in message or 'rate limit' in message.lower():limit=message
        for name,value in (event.get('usage') or {}).items():
            if isinstance(value,int):tokens[name]=tokens.get(name,0)+value
    st=state(work);total=st['tokens']
    for name,value in tokens.items():total[name]=total.get(name,0)+value
    st['sessions'].append({'n':n,'role':role,'thread':thread,'resumed':bool(resume),'started':started,'ended':now(),'exit':code,'tokens':tokens,**({'limit':limit} if limit else {})})
    state(work,sessions=st['sessions'],tokens=total)
    if limit:LIMIT.set()
    return thread,limit

MAKER="""You are the maker of the packet for the work {work} in the planting line of the Aesthetics bonsai. Your working directory is the packet. Read ../../AGENTS.md and ../../MAKER.md now and follow MAKER.md to its end. Stop only when `python3 ../../kit.py gate .` prints `gate: open`, or when you have written blocked.json as the card describes."""
MAKER_ON="""You are the maker of the packet in this directory (../../AGENTS.md, ../../MAKER.md). The gate is still shut. These are its lines:

{gate}

Settle each line as ../../MAKER.md and ../../AGENTS.md say, then run `python3 ../../kit.py gate .` again. Stop only at `gate: open` or at blocked.json."""
MAKER_BACK="""You are the maker of the packet in this directory (../../AGENTS.md, ../../MAKER.md). The second reader has read the packet; the answer is in review.json. What is a yes stays exactly as it is. For every conception, link or page that is not a yes, read the note and the place again in the original, then mend it so that the place says it: a thesis that says more than the sentences is rewritten to say what they say; a thesis that sets two things side by side becomes two conceptions, each with its own place and both grown from the same register reading; a cut that falls short is cut again longer with kit.py cut; a text that differs from the page is corrected with kit.py correct. A reading under `left` that review.json marks `holds: false` is borne by the work at the place its note names: grow it. Only where the work does not bear the reading at all do its register keys go under `left` with the reason. Where you find the second reader mistaken, keep the record and say why in its `note`. Then run `python3 ../../kit.py gate .` until it prints `gate: open`."""
BLOCK="""You are a maker in the planting line of the Aesthetics bonsai; your working directory is the packet of the work {work}. An earlier maker blocked this packet (blocked.json). Its reason: {reason}

Read ../../AGENTS.md and ../../MAKER.md, then test that block yourself: look for what it says is missing or failing. If the block does not hold, delete blocked.json and make the packet by MAKER.md to its end (`python3 ../../kit.py gate .` prints `gate: open`). If it holds, leave blocked.json and add under `tried` what you looked at."""
RETAKE="""This packet has been through the line before. Under `left` in reading.json, an entry marked `disputed` holds a conception or a link that a second reader did not confirm, with the reader's note. Take each of them up again: read the note and the place in the original; where the work bears the reading in a narrower thesis, a longer cut or two conceptions, grow it again by MAKER.md and take the entry out of `left`; where it does not, leave the entry as it is. Leave what is already in the reading as it is unless the gate asks otherwise."""
SECOND="""You are the second reader of the packet for the work {work} in the planting line of the Aesthetics bonsai. You did not make this packet. Your working directory is the packet. Read ../../AGENTS.md and ../../SECOND-READER.md now and follow SECOND-READER.md to its end. Stop only when `python3 ../../kit.py gate . --second` prints `gate: open`."""
SECOND_ON="""You are the second reader of the packet in this directory (../../AGENTS.md, ../../SECOND-READER.md). The gate is still shut. These are its lines:

{gate}

Settle each line as ../../SECOND-READER.md says, then run `python3 ../../kit.py gate . --second` again."""

def have(work,name):return os.path.exists(os.path.join(PACKETS,work,name))
def until_open(work,role,first,again,turns,second=False,thread=None):
    """Turns of one reader until the gate opens, a blocked.json appears, the turns run out or the usage limit ends them."""
    prompt=first
    for turn in range(turns):
        thread,limit=codex(work,role,prompt,resume=thread if turn or thread else None)
        if limit:return thread,'limit'
        code,text=kit('gate',os.path.join(PACKETS,work),*(['--second'] if second else []))
        if not second and have(work,'blocked.json'):
            if code==0:return thread,'blocked'                    # a block the gate does not accept goes back like any shut gate
        elif code==0:kit('stamp',os.path.join(PACKETS,work),'second' if second else 'maker',MODEL,EFFORT);return thread,'open'
        prompt=again.format(gate='\n'.join(text.splitlines()[:60]))
    return thread,'stuck'

def disputed(work):
    path=os.path.join(PACKETS,work,'reading.json');return os.path.exists(path) and any(item.get('disputed') for item in json.load(open(path)).get('left') or [])

def run(work,retake=False):
    if retake:
        for name in('accepted.json',):
            if have(work,name):os.remove(os.path.join(PACKETS,work,name))
    if have(work,'accepted.json'):return 'accepted'
    if not have(work,'blocked.json') and not(have(work,'places.json') and have(work,'reading.json')):   # a new packet, or one whose block was lifted
        code,text=kit('open',work)
        if code:state(work,result='no-task',note=text[-300:]);return 'no-task'
    state(work,result='running',started=now())
    def end(result,**more):state(work,result=result,**more);return result
    maker=None
    if have(work,'blocked.json'):                                  # a block is looked at once more by a fresh reader before it stands
        block=json.load(open(os.path.join(PACKETS,work,'blocked.json')))
        if state(work).get('block_checked') or block.get('ground')=='rights' and kit('gate',os.path.join(PACKETS,work))[0]==0:return end('blocked')
        state(work,block_checked=True);maker,got=until_open(work,'maker',BLOCK.format(work=work,reason=block.get('reason')),MAKER_ON,MAKER_TURNS)
        if got!='open':return end({'limit':'limit','blocked':'blocked'}.get(got,'maker-stuck'))
    elif (retake and disputed(work)) or kit('gate',os.path.join(PACKETS,work))[0]:       # a packet whose gate is already open goes straight to its second reader
        maker,got=until_open(work,'maker',MAKER.format(work=work)+('\n\n'+RETAKE if retake and disputed(work) else ''),MAKER_ON,MAKER_TURNS)
        if got=='blocked':return run(work)                       # the block gets its second look
        if got!='open':return end('limit' if got=='limit' else 'maker-stuck')
    for round_ in range(1,RETURNS+2):
        review=os.path.join(PACKETS,work,'review.json')
        if os.path.exists(review):os.replace(review,out(work,f'review-{round_-1}.json'))
        _,got=until_open(work,'second',SECOND.format(work=work),SECOND_ON,SECOND_TURNS,second=True)
        if got!='open':return end('limit' if got=='limit' else 'second-stuck')
        if json.load(open(review)).get('accept'):
            code,text=kit('accept',os.path.join(PACKETS,work),'--by','planting-line')
            return end('accepted' if code==0 else 'accept-failed',note=text[-300:])
        if round_<=RETURNS:
            maker,got=until_open(work,'maker',MAKER_BACK,MAKER_ON,MAKER_TURNS,thread=maker)
            if got!='open':return end({'limit':'limit','blocked':'blocked'}.get(got,'maker-stuck'))
    answer=json.load(open(review))
    if any(item.get('reads')!='as the packet' for item in answer.get('images') or []):return end('page-disputed')
    code,text=kit('settle',os.path.join(PACKETS,work))
    reading=json.load(open(os.path.join(PACKETS,work,'reading.json')))
    if code or not reading['conceptions']:return end('disputed',note=text[-300:])
    basis=f"Made by {reading['reader']['id']}; read a second time by {answer['reader']['id']}, who confirms the {len(reading['conceptions'])} conceptions and {len(reading['links'])} links kept. What the second reader does not confirm after {RETURNS} returns to the maker is under left, marked disputed."
    code,text=kit('accept',os.path.join(PACKETS,work),'--by','planting-line','--basis',basis)
    return end('accepted-in-part' if code==0 else 'accept-failed',note=text[-300:])

def roomy():
    """Wait for memory before a new reader starts: the line shares a laptop."""
    for _ in range(60):
        free=next((int(line.split()[1])//1024 for line in open('/proc/meminfo') if line.startswith('MemAvailable')),99999)
        if free>4000:return
        time.sleep(30)

def line(args):
    kit('queue');queue=json.load(open(os.path.join(HERE,'queue.json')))['works']
    works=args.works.split(',') if args.works else [row['work'] for row in queue if row['wave']==args.wave]
    works=[work for work in works if args.retake or (not have(work,'accepted.json') and not have(work,'blocked.json'))][:args.count or None]
    print(f'{now()} line: {len(works)} works, {args.max} at a time, {MODEL} {EFFORT}',flush=True)
    def one(work):
        if LIMIT.is_set():return work,'not-started'
        roomy()
        try:result=run(work,retake=args.retake)
        except Exception as error:result=f'error: {error}';state(work,result=result)
        print(f"{now()} {result:16} {work}  tokens {state(work)['tokens']}",flush=True);return work,result
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.max) as pool:results=list(pool.map(one,works))
    count={}
    for _,result in results:count[result]=count.get(result,0)+1
    print(f'{now()} line ended: '+', '.join(f'{name} {n}' for name,n in count.items())+(' — the usage limit ended it' if LIMIT.is_set() else ''),flush=True)

def main():
    p=argparse.ArgumentParser(description=__doc__,formatter_class=argparse.RawDescriptionHelpFormatter);s=p.add_subparsers(dest='cmd',required=True)
    q=s.add_parser('run');q.add_argument('work');q.add_argument('--retake',action='store_true')
    q=s.add_parser('line');q.add_argument('--max',type=int,default=2);q.add_argument('--works');q.add_argument('--count',type=int);q.add_argument('--wave',default='first');q.add_argument('--retake',action='store_true',help='run the named works again and take up what was left disputed')
    args=p.parse_args()
    if args.cmd=='run':print(run(args.work,retake=args.retake))
    else:line(args)

if __name__=='__main__':main()
