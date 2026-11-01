"""Builds bonsai/antiquity/fragments.json (tos.bonsai.fragments.v1) from the places the external-fragments agent
gathered (ext2/antiquity-fragments.json), with reader-facing links, short witness names and the rights records named
by jurisdiction. Only the places the stand cites are kept. Usage: python3 -I antiquity_fragments.py"""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, re
S = REVIEW
REPO = REPO_ROOT
src = json.load(open(f'{S}/ext2/antiquity-fragments.json'))
needed = json.load(open(f'{S}/ext2/places-needed.json'))

def blob(raw):
    """A raw GitHub file as GitHub shows it."""
    m = re.match(r'^https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/(.+)$', raw)
    return f'https://github.com/{m.group(1)}/{m.group(2)}/blob/{m.group(3)}/{m.group(4)}'
IA = 'https://archive.org/details/'
WS = 'https://en.wikisource.org/wiki/Page:Ad_Lucilium_epistulae_morales,_volume_1.djvu/'
# The stand's names for each witness, the page a reader opens, and the copies the text was taken from or read against.
WITNESSES = {
    'de-anima': {
        'grc': {'url': IA + 'aristotelisoper03arisuoft', 'sources': ['transcription', IA + 'aristotelisopera01arisuoft'], 'short': {'ru': 'текст Беккера, Оксфорд, 1837', 'en': 'Bekker’s text, Oxford, 1837'}},
        'en': {'url': IA + 'aristotledeanima0000rdhi', 'short': {'ru': 'пер. Р. Д. Хикса, 1907', 'en': 'tr. R. D. Hicks, 1907'}}},
    'letters': {
        'la': {'url': IA + 'adluciliumepistu01sene', 'sources': ['transcription'], 'short': {'ru': 'лат. текст изд. Лёба, 1917', 'en': 'Latin of the Loeb edition, 1917'}},
        'en': {'url': 'https://en.wikisource.org/wiki/Moral_letters_to_Lucilius', 'sources': [IA + 'adluciliumepistu01sene'], 'short': {'ru': 'пер. Р. М. Гаммери, 1917', 'en': 'tr. Richard M. Gummere, 1917'}}},
    'memorabilia': {
        'grc': {'url': IA + 'operaomniarecogn02xenouoft', 'sources': ['transcription'], 'short': {'ru': 'текст Э. К. Маршана, Оксфорд', 'en': 'Marchant’s Oxford text'}},
        'en': {'url': 'https://www.gutenberg.org/ebooks/1177', 'sources': [IA + 'WorksOfXenophonV.3Pt.1'], 'short': {'ru': 'пер. Г. Г. Дейкинса, 1897', 'en': 'tr. H. G. Dakyns, 1897'}}},
    'outlines': {
        'grc': {'url': IA + 'rsoperarecensuit01sextuoft', 'sources': ['transcription'], 'short': {'ru': 'изд. Г. Мучмана, 1912', 'en': 'ed. H. Mutschmann, 1912'}},
        'en': {'url': 'https://www.gutenberg.org/ebooks/17556', 'sources': [IA + 'cu31924029002561'], 'short': {'ru': 'пер. М. М. Патрик, 1899', 'en': 'tr. Mary Mills Patrick, 1899'}},
        'en-bury': {'url': 'transcription', 'short': {'ru': 'пер. Р. Г. Бери, 1933', 'en': 'tr. R. G. Bury, 1933'}}},
}
def rights_of(r):
    out = {'basis': r['basis']}
    for a, b in (('ru', 'russia'), ('us', 'usa'), ('eu', 'eu')):
        if r.get(a): out[b] = r[a]
    if r.get('reasoning'): out['reasoning'] = r['reasoning']
    return out
year = lambda v: int(v) if isinstance(v, str) and re.fullmatch(r'\d{4}', v) else v
works, places, problems = {}, {}, []
for key, work in src['works'].items():
    witnesses, absent = {}, []
    for lang, w in work['witnesses'].items():
        if not w or w.get('url') in (None, 'None', ''):
            absent.append({'language': lang, 'reason': f"{w.get('edition')} — {w['rights'].get('reasoning') or w['rights'].get('ru')}" if w else 'none found'})
            continue
        over = WITNESSES[key][lang]
        transcription = blob(w['url']) if 'raw.githubusercontent.com' in w['url'] else w['url']
        entry = {'edition': w['edition'], **({'translator': w['translator']} if w.get('translator') not in (None, 'None') else {}), 'year': year(w.get('year')),
                 'url': transcription if over['url'] == 'transcription' else over['url'],
                 **({'sources': [transcription if s == 'transcription' else s for s in over['sources']]} if over.get('sources') else {}),
                 'licence': w.get('license'), 'attribution': w['attribution'], 'rights': rights_of(w['rights']), 'short': over['short']}
        if 'transcription' not in over.get('sources', []) and over['url'] != 'transcription' and transcription != entry['url']:
            entry.setdefault('sources', []).insert(0, transcription)
        witnesses[lang] = {k: v for k, v in entry.items() if v not in (None, 'None')}
    works[key] = {'author': work['author'], 'title': work['title'], 'original': work['original'], 'witnesses': witnesses, **({'absent': absent} if absent else {})}
for p in src['places']:
    key, loc = p['work'], p['loc']
    if loc not in needed.get(key, []): problems.append(f'{key} {loc} is not cited'); continue
    entry = {'label': p['label'], 'texts': {lang: t for lang, t in p['texts'].items() if t}}
    if p.get('witness'): entry['witness'] = p['witness']
    refs = [[lang, ref] for lang, ref in (p.get('source_refs') or {}).items() if ref]
    # Seneca's English links its own page of the Wikisource transcription, with the scan beside it.
    if key == 'letters':
        page = re.search(r'volume 1\.djvu/(\d+)', (p.get('source_refs') or {}).get('en') or '')
        if page: entry['links'] = {'en': WS + page.group(1)}
    if refs: entry['refs'] = refs
    if p.get('note'): entry['note'] = p['note']
    places.setdefault(key, {})[loc] = entry
for key, locs in needed.items():
    for loc in locs:
        if loc not in places.get(key, {}): problems.append(f'cited place {key} {loc} has no entry')
module = {
    'schema': 'tos.bonsai.fragments.v1',
    'posture': {'ru': 'Точный текст мест других произведений, на которые опирается стенд, в оригинале и переводах; у каждого свидетеля — издание, источник, лицензия и основание показа. Охраняемое произведение показано короткой цитатой или только адресом места.',
                'en': 'The exact text of the places of other works the stand rests on, in the original and in translations; every witness with its edition, source, licence and basis for showing it. A protected work is shown by a short quotation or by the address of the place alone.'},
    'provenance': {'gathered': '2026-10-30', 'method': 'Exact text read in the shell from the digital copies named per witness and checked against the page images of the printed editions; where they differ, the print decides and the place notes the change. Greek line ranges follow Bekker’s Berlin edition (a word broken across lines counts to the line where it begins); a translation gives the smallest run of whole sentences that renders the place.'},
    'works': works, 'places': places}
json.dump(module, open(f'{REPO}/bonsai/antiquity/fragments.json', 'w'), ensure_ascii=False, indent=1)
open(f'{REPO}/bonsai/antiquity/fragments.json', 'a').write('\n')
print('works', len(works), 'places', sum(len(v) for v in places.values()), 'problems', problems)
for key, w in works.items(): print(key, {l: (x['rights']['basis'], x['url']) for l, x in w['witnesses'].items()}, [a['language'] for a in w.get('absent', [])])
