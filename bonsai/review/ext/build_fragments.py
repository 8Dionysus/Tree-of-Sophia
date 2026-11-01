"""Builds a stand's fragments.json (tos.bonsai.fragments.v1) from an agent's fetched places.
  python3 -I build_fragments.py <places.json> <map.json> <module.json> <stand content cites (json list of [w, loc])>

<map.json>: {"works": {"<agent work id>": "<stand work key>"}, "places": {"<stand work key>": {"<agent loc>": "<stand loc>"}},
             "locator_only": {"<stand work key>": {"<stand loc>": {"label": {"ru": "...", "en": "..."}, "url": "..."}}},
             "short": {"<stand work key>": {"<lang>": {"ru": "...", "en": "..."}}}}
Every place the stand cites must end up in the module, with its exact text where the rights allow it and by
its locator alone where they do not."""
import json, sys
places_in, mapping, out_path, cited_path = sys.argv[1:5]
src = json.load(open(places_in)); m = json.load(open(mapping)); cited = json.load(open(cited_path))
works, places = {}, {}
for agent_id, work in src['works'].items():
    key = m['works'].get(agent_id)
    if not key: continue
    witnesses = {}
    for lang, w in (work.get('witnesses') or {}).items():
        if not w: continue
        short = m.get('short', {}).get(key, {}).get(lang)
        entry = {'edition': w['edition'], 'year': w.get('year'), 'url': w.get('url'), 'licence': w.get('license') or w.get('licence'),
                 'attribution': w.get('attribution'), 'rights': w['rights']}
        if w.get('translator'): entry['translator'] = w['translator']
        if short: entry['short'] = short
        if w.get('publisher_page'): entry['publisher_page'] = w['publisher_page']
        witnesses[lang] = {k: v for k, v in entry.items() if v is not None}
    works[key] = {'author': work['author'], 'title': {'ru': work['title']['ru'], 'en': work['title']['en']}, 'original': work.get('original') or next(iter(witnesses), None), 'witnesses': witnesses}
for place in src['places']:
    key = m['works'].get(place['work'])
    if not key: continue
    loc = m.get('places', {}).get(key, {}).get(place['loc'], place['loc'])
    texts = {lang: (t if t else None) for lang, t in (place.get('texts') or {}).items() if lang in works[key]['witnesses']}
    entry = {'label': place['label'], 'texts': texts}
    if place.get('source_refs'): entry['source_refs'] = place['source_refs']
    if place.get('note'): entry['note'] = place['note']
    places.setdefault(key, {})[loc] = entry
for key, locs in (m.get('locator_only') or {}).items():
    for loc, entry in locs.items():
        places.setdefault(key, {}).setdefault(loc, {'label': entry['label'], 'texts': {}, **({'url': entry['url']} if entry.get('url') else {})})
missing = sorted({f'{w} {loc}' for w, loc in cited if loc not in places.get(w, {})})
module = {'schema': 'tos.bonsai.fragments.v1',
          'posture': {'ru': 'Точный текст мест других произведений, на которые опирается стенд, в оригинале и переводах; у каждого свидетеля — издание, источник, лицензия и основание показа. Охраняемое произведение показано короткой цитатой или только адресом места.',
                      'en': 'The exact text of the places of other works the stand rests on, in the original and in translations; every witness with its edition, source, licence and basis for showing it. A protected work is shown by a short quotation or by the address of the place alone.'},
          'works': works, 'places': places}
json.dump(module, open(out_path, 'w'), ensure_ascii=False, indent=1)
print('works', len(works), 'places', sum(len(v) for v in places.values()), 'missing cited places', missing)
