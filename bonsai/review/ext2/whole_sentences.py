"""De anima places are Bekker line ranges; the Greek was cut at line ends, mid-sentence. Like every other cited
place, each now shows the whole sentences its lines stand in (Greek sentences end at "." and ";"), the added
words taken from the same First1KGreek transcription of Bekker's text, around the agent's checked core.
Usage: python3 -I whole_sentences.py [--write]"""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, re, sys, unicodedata
S = REVIEW
REPO = REPO_ROOT
xml = open(f'{S}/ext2/sources/github-first1k/tlg0086.tlg002.1st1K-grc1.xml', encoding='utf-8').read()
body = xml[xml.find('<body'):]
body = re.sub(r'<note[^>]*>.*?</note>', ' ', body, flags=re.S)
body = re.sub(r'<[^>]+>', ' ', body)
text = unicodedata.normalize('NFC', re.sub(r'\s+', ' ', body)).strip()
END = re.compile(r'[.;;](?=\s)')
mod = json.load(open(f'{REPO}/bonsai/antiquity/fragments.json'))
norm = lambda t: unicodedata.normalize('NFC', re.sub(r'\s+', ' ', t)).strip()
report = []
for loc, place in mod['places']['de-anima'].items():
    core = norm(place['texts']['grc'])
    i = text.find(core)
    if i < 0:
        # The core may hold a reading corrected from the print; anchor on its first and last words.
        head, tail = core[:25], core[-25:]
        a, b = text.find(head), text.find(tail)
        if a < 0 or b < 0 or b < a: report.append((loc, 'not found')); continue
        i, j = a, b + len(tail)
        before, after = text[:i], text[j:]
    else:
        before, after = text[:i], text[i + len(core):]
    # The start: just after the last sentence end before the core (unless the core starts a sentence).
    ends = [m.end() for m in END.finditer(before)]
    start = ends[-1] if ends else 0
    lead = before[start:].lstrip()
    m = END.search(after) if not re.search(r'[.;;]$', core) else None
    trail = after[:m.end()] if m else ''
    whole = (lead + core + trail).strip()
    whole = re.sub(r'\s+([.,;·])', r'\1', whole)
    report.append((loc, len(core), len(whole), lead[:40], trail[-40:]))
    place['texts']['grc'] = whole
for r in report: print(r)
if '--write' in sys.argv:
    mod['provenance']['method'] += ' A place named by Bekker lines shows the whole sentences those lines stand in; the words around the checked lines come from the same transcription.'
    json.dump(mod, open(f'{REPO}/bonsai/antiquity/fragments.json', 'w'), ensure_ascii=False, indent=1)
    open(f'{REPO}/bonsai/antiquity/fragments.json', 'a').write('\n')
    print('written')
