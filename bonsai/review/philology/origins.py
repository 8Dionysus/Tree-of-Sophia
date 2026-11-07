#!/usr/bin/env python3
"""Gather the origin of every lemma of a bonsai's witnesses.

    python3 origins.py <repo> <bonsai> --lexicon-dir <dir>

Reads the bonsai's word layer (`tokens/`), witnesses, spelling and terms, and
the Wiktionary indexes built by `lexicon.py`; writes `origins.json` and
`origins/<language>-<shard>.json`. The record of a lemma says where the word
comes from:

- `origin`: its descent, oldest form first, as Wiktionary's etymology names it
  (each step a relation, a language code and the form), the parts it is made
  of, and its cognates in other languages;
- `spelling`: how the first editions write it, where that differs from today;
- `renders` (a translation's word): the words of the original it renders, by
  Dice association over the bonsai's sentence alignment; `renderings` (the
  original's word): the translations' words for it;
- `first`: its first place in the book;
- `term`: the bonsai term it is, where it is one.

The descent is a fact Wiktionary records (CC BY-SA 4.0, Wiktionary
contributors); the manifest names the extract it was taken from.
"""
import argparse, glob, json, os, re, sqlite3, sys, unicodedata
from collections import Counter, defaultdict
from urllib.parse import quote

SHARDS = 32
CONTENT = {'NOUN', 'PROPN', 'VERB', 'ADJ', 'ADV'}
POS_UPOS = {'noun': {'NOUN', 'PROPN'}, 'name': {'PROPN', 'NOUN'}, 'verb': {'VERB', 'AUX'}, 'adj': {'ADJ', 'ADV', 'VERB'},
            'adv': {'ADV', 'ADJ'}, 'pron': {'PRON', 'DET'}, 'det': {'DET', 'PRON'}, 'article': {'DET'}, 'prep': {'ADP'},
            'conj': {'CCONJ', 'SCONJ'}, 'particle': {'PART', 'ADV'}, 'intj': {'INTJ'}, 'num': {'NUM', 'ADJ'}}
RELATION = {'inh': 'inherited', 'inh+': 'inherited', 'der': 'derived', 'der+': 'derived', 'bor': 'borrowed',
            'bor+': 'borrowed', 'lbor': 'borrowed', 'slbor': 'borrowed', 'cal': 'calque', 'clq': 'calque',
            'calque': 'calque', 'pcal': 'calque', 'sl': 'semantic loan', 'translit': 'transliterated'}
PARTS = {'compound', 'com', 'affix', 'af', 'prefix', 'pre', 'suffix', 'suf', 'confix', 'con', 'blend'}
COGNATE = {'cog', 'cognate'}


def fnv1a(text):
    h = 0x811c9dc5
    for byte in text.encode('utf-8'):
        h ^= byte
        h = (h * 0x01000193) & 0xffffffff
    return h


def shard_of(lemma):
    return fnv1a(lemma) % SHARDS


def fold(word, lang):
    text = unicodedata.normalize('NFD', word)
    text = ''.join(ch for ch in text if ch not in '́̀')
    text = unicodedata.normalize('NFC', text)
    return text.replace('ё', 'е').replace('Ё', 'Е') if lang == 'ru' else text


def language_name(expansion, term):
    expansion = re.sub(r'\s+', ' ', expansion or '').strip()
    term = (term or '').strip()
    if term and term in expansion:
        name = expansion[:expansion.index(term)].strip()
    else:
        name = re.split(r'\s[*\wͰ-ϿЀ-ӿ]', expansion, maxsplit=1)[0]
    name = re.sub(r'^(?:Inherited from|Derived from|Borrowed from|Calque of|From)\s+', '', name, flags=re.I).strip(' ,(')
    return name or None


def parse_templates(templates):
    chain, parts, cognates = [], [], []
    for t in templates or []:
        name, args, expansion = t.get('name'), t.get('args') or {}, t.get('expansion') or ''
        if name in RELATION and args.get('3'):
            term = args.get('3').strip()
            chain.append({'relation': RELATION[name], 'language': language_name(expansion, term) or args.get('2'),
                          'code': args.get('2'), 'term': term, 'gloss': args.get('t') or args.get('4') or args.get('gloss') or None})
        elif name in PARTS:
            for key in sorted((k for k in args if k.isdigit() and k != '1'), key=int):
                if args[key].strip():
                    parts.append(args[key].strip())
        elif name in COGNATE and args.get('2'):
            cognates.append({'language': language_name(expansion, args.get('2')) or args.get('1'), 'code': args.get('1'),
                             'term': args.get('2').strip()})
    seen, unique = set(), []
    for step in chain:
        key = (step['code'], step['term'])
        if key not in seen:
            seen.add(key)
            unique.append({k: v for k, v in step.items() if v})
    return unique, parts, cognates[:12]


class Wiktionary:
    def __init__(self, path, lang):
        self.lang = lang
        self.db = sqlite3.connect(path)
        self.meta = dict(self.db.execute('select key, value from meta'))

    def origins(self, lemma, upos):
        """The descent of a lemma from its Wiktionary entries that fit its part of speech, oldest form first."""
        rows = self.db.execute('select word, pos, etymology_templates, spelling_of, form_of from entry where key=?',
                               (fold(lemma, self.lang),)).fetchall()
        if not rows and lemma[:1].isupper():
            rows = self.db.execute('select word, pos, etymology_templates, spelling_of, form_of from entry where key=?',
                                   (fold(lemma[:1].lower() + lemma[1:], self.lang),)).fetchall()
        out, seen = [], set()
        for fits in (True, False):
            for word, pos, templates, spelling_of, form_of in rows:
                if form_of or spelling_of or (upos in POS_UPOS.get(pos, set())) != fits:
                    continue
                chain, parts, cognates = parse_templates(json.loads(templates or '[]'))
                item = {}
                if chain:
                    item['chain'] = [[step['relation'], step.get('code'), step['term']] for step in reversed(chain)]
                if parts:
                    item['parts'] = parts
                if cognates:
                    item['cognates'] = [[c.get('code'), c['term']] for c in cognates[:8]]
                key = json.dumps(item, sort_keys=True)
                if item and key not in seen:
                    seen.add(key)
                    out.append(item)
            if out:
                break
        return out[:2]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('bonsai')
    ap.add_argument('--lexicon-dir', required=True)
    args = ap.parse_args()
    home = os.path.join(args.repo, 'bonsai', args.bonsai)
    witnesses = json.load(open(os.path.join(home, 'witnesses.json')))
    order = [c['key'] for c in witnesses['chapters']]
    terms = json.load(open(os.path.join(home, 'terms.json')))
    spelling = json.load(open(os.path.join(home, 'spelling.json')))
    original = witnesses.get('pivot') if isinstance(witnesses.get('pivot'), str) else 'de'
    lex = {lang: Wiktionary(os.path.join(args.lexicon_dir, f'{lang}.sqlite'), lang)
           for lang in ('de', 'ru', 'en') if os.path.exists(os.path.join(args.lexicon_dir, f'{lang}.sqlite'))}

    stats = {lang: defaultdict(lambda: {'n': 0, 'first': None, 'chapters': set(), 'upos': Counter(), 'spellings': Counter()})
             for lang in ('de', 'ru', 'en')}
    pair_lemmas = {lang: [] for lang in ('ru', 'en')}  # per aligned sentence pair: (original lemmas, translation lemmas)
    for key in order:
        path_t = os.path.join(home, 'tokens', f'{key}.json')
        if not os.path.exists(path_t):
            continue
        layer = json.load(open(path_t))
        chapter = json.load(open(os.path.join(home, 'witnesses', f'{key}.json')))
        entries = layer['entries']
        by_unit = {}
        for lang, block in layer['witnesses'].items():
            for index, unit in enumerate(chapter['units'][lang]):
                rows = block['units'].get(unit[0], [])
                by_unit[(lang, index)] = rows
                for row in rows:
                    lemma, upos = entries[row[2]][0], entries[row[2]][1]
                    if upos == 'PUNCT':
                        continue
                    record = stats[lang][lemma]
                    record['n'] += 1
                    record['upos'][upos] += 1
                    record['chapters'].add(key)
                    if lang == 'de' and len(row) > 6:  # the first editions' own spelling of a word read in today's
                        source = re.sub(r'¬\s*', '', ''.join(list(unit[2])[row[0]:row[1]]))
                        if source in spelling.get('forms', {}):
                            record['spellings'][source] += 1
                    if record['first'] is None:
                        record['first'] = [key, unit[0], row[0], row[1]]

        def lemmas_in(lang, spans):
            out = set()
            for k, a, b in spans:
                for row in by_unit.get((lang, k), []):
                    if row[0] >= a and row[1] <= b and entries[row[2]][1] in CONTENT:
                        out.add(entries[row[2]][0])
            return out

        for row in chapter.get('rows', []):
            for lang in ('ru', 'en'):
                for left, right in row.get('sentences', {}).get(lang, []):
                    pair_lemmas[lang].append((lemmas_in(original, left), lemmas_in(lang, right)))

    # Dice association between original and translation lemmas over aligned sentence pairs.
    renderings, renders = defaultdict(dict), defaultdict(dict)
    for lang, pairs in pair_lemmas.items():
        c_o, c_t, joint = Counter(), Counter(), defaultdict(Counter)
        for left, right in pairs:
            c_o.update(left)
            c_t.update(right)
            for l in left:
                joint[l].update(right)
        best = {}
        for l, row in joint.items():
            if c_o[l] < 2:
                continue
            scored = sorted(((2 * n / (c_o[l] + c_t[r]), r, n) for r, n in row.items() if n >= 2), reverse=True)
            best[l] = [(r, d) for d, r, n in scored if d >= 0.2][:4]
        reverse = defaultdict(Counter)
        for l, candidates in best.items():
            if not candidates:
                continue
            counted, unmatched = Counter(), 0
            for left, right in pairs:
                if l not in left:
                    continue
                match = next((r for r, _ in candidates if r in right), None)
                if match:
                    counted[match] += 1
                    reverse[match][l] += 1
                else:
                    unmatched += 1
            if counted:
                renderings[l][lang] = {'words': [[r, n] for r, n in counted.most_common()], 'other': unmatched}
        for r, row in reverse.items():
            renders[lang][r] = [[l, n] for l, n in row.most_common(4)]

    term_of = {}
    for term in terms.get('terms', []):
        lemma = term['original']['lemma']
        # a term spelled as the first editions spell it (Thier) is the lemma in today's spelling (Tier)
        term_of[('de', spelling.get('forms', {}).get(lemma, {}).get('modern', lemma))] = term['id']
        for lang in ('ru', 'en'):
            tos = term.get('renderings', {}).get(lang, {}).get('tos', {}).get('term')
            if tos:
                term_of.setdefault((lang, tos), term['id'])

    counts = Counter()
    for lang, table in stats.items():
        shards = defaultdict(dict)
        for lemma, record in table.items():
            upos = record['upos'].most_common(1)[0][0]
            item = {'first': record['first']}
            if lang in lex and upos not in ('PUNCT', 'NUM', 'SYM', 'X'):  # über- needs its origin as a part of Übermensch
                origin = lex[lang].origins(lemma, upos)
                if origin:
                    item['origin'] = origin
                    counts[f'{lang}_with_origin'] += 1
            if record['spellings']:
                item['spelling'] = [form for form, _ in record['spellings'].most_common()]
            if lang == original and renderings.get(lemma):
                item['renderings'] = {target: value['words'][:3] for target, value in renderings[lemma].items()}
            if lang != original and renders.get(lang, {}).get(lemma):
                item['renders'] = renders[lang][lemma][:3]
            if (lang, lemma) in term_of:
                item['term'] = term_of[(lang, lemma)]
            shards[shard_of(lemma)][lemma] = item
            counts[f'{lang}_lemmas'] += 1
        os.makedirs(os.path.join(home, 'origins'), exist_ok=True)
        for old in glob.glob(os.path.join(home, 'origins', f'{lang}-*.json')):
            os.remove(old)
        for shard, items in shards.items():
            with open(os.path.join(home, 'origins', f'{lang}-{shard:02d}.json'), 'w') as fh:
                json.dump({'schema': 'tos.bonsai.origins.shard.v1', 'language': lang, 'shard': shard,
                           'lemmas': dict(sorted(items.items()))}, fh, ensure_ascii=False, separators=(',', ':'))
                fh.write('\n')
    manifest = {
        'schema': 'tos.bonsai.origins.v1', 'work': witnesses.get('work'), 'original': original,
        'posture': {'ru': 'Откуда пришло каждое слово свидетелей: его родословная от древнейшей формы, части, из которых оно сложено, родственные слова, написание первых изданий, слово оригинала, которое передаёт перевод, и первое место в книге.',
                    'en': "Where each word of the witnesses comes from: its descent from its oldest form, the parts it is made of, its cognates, the first editions' spelling, the word of the original a translation renders, and its first place in the book."},
        'shards': {'count': SHARDS, 'key': 'FNV-1a 32-bit over the UTF-8 bytes of the lemma, modulo the count',
                   'path': 'origins/<language>-<shard, two digits>.json'},
        'renderings': {'method': 'Dice association of content lemmas over the aligned sentence pairs of bonsai/lib/align.mjs (at least two shared pairs, Dice ≥ 0.2); at each place of the lemma the strongest associated word present in the aligned sentence',
                       'posture': 'association over an unreviewed automatic alignment; a measure of co-occurrence'},
        'sources': {lang: {'label': 'English Wiktionary via kaikki.org (Wiktextract)', 'url': f"https://kaikki.org/dictionary/{ {'de': 'German', 'ru': 'Russian', 'en': 'English'}[lang]}/",
                           'extract_sha256': w.meta.get('extract_sha256'), 'licence': 'CC BY-SA 4.0',
                           'licence_url': 'https://creativecommons.org/licenses/by-sa/4.0/', 'attribution': 'Wiktionary contributors'}
                    for lang, w in lex.items()},
        'licence': 'CC BY-SA 4.0', 'counts': dict(counts)}
    with open(os.path.join(home, 'origins.json'), 'w') as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=1)
        fh.write('\n')
    print(json.dumps(manifest['counts']))


if __name__ == '__main__':
    sys.exit(main())
