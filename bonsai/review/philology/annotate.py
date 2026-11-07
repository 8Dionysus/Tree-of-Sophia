#!/usr/bin/env python3
"""Annotate a bonsai's witnesses word by word: spelling, lemma, grammar, syntax.

    python3 annotate.py <repo> <bonsai> --lexicon-dir <dir> [--chapters I.01 ...] [--languages de ru en]

Writes, inside the bonsai, `spelling.json` (each historical form of the
original with today's spelling and the ground of that spelling) and
`tokens.json` with `tokens/<chapter>.json` (every word of every unit of every
witness at its exact place). The text of a witness is never changed: offsets
count the Unicode code points of the unit's exact text, and a word written
across a line break keeps its whole span.

Three layers, each kept separately:

1. Joins and spelling. A word the German first editions break at a line end
   with "¬" is read whole. A historical spelling takes today's spelling when
   Wiktionary files it as an obsolete or Swiss spelling of that word, or when
   the orthographic rules below give a word Wiktionary knows (a compound
   counts when both its parts are known words). The ground is recorded.
2. Grammar in context. Stanza reads the unit in today's spelling and gives
   each word its part of speech, features, lemma and syntactic head.
3. Lemma. The lemma is checked against Wiktionary's forms: when Stanza's lemma
   is among the lemmas Wiktionary gives for the form with that part of speech,
   both agree (`a`); when Stanza's lemma is not, Wiktionary's lemma that fits
   the part of speech and features is taken (`l`); when Wiktionary does not
   know the form, Stanza's lemma stands alone (`m`).

Everything here is a machine proposal for the stand, unreviewed.
"""
import argparse, glob, hashlib, json, os, re, sqlite3, sys, time, unicodedata
from collections import Counter, defaultdict
from itertools import combinations

CP = lambda text: list(text)  # code points


# ---------------------------------------------------------------- lexicon

OLD_TAGS = {'obsolete', 'alternative', 'archaic', 'dated', 'nonstandard'}
POS_UPOS = {
    'noun': {'NOUN', 'PROPN'}, 'name': {'PROPN', 'NOUN'}, 'verb': {'VERB', 'AUX'}, 'adj': {'ADJ', 'ADV', 'VERB'},
    'adv': {'ADV', 'ADJ'}, 'pron': {'PRON', 'DET'}, 'det': {'DET', 'PRON'}, 'article': {'DET'}, 'prep': {'ADP'},
    'postp': {'ADP'}, 'conj': {'CCONJ', 'SCONJ'}, 'particle': {'PART', 'ADV'}, 'intj': {'INTJ'}, 'num': {'NUM', 'ADJ'},
    'contraction': {'ADP', 'DET'}, 'prep_phrase': {'ADP'}, 'phrase': set(), 'suffix': set(), 'prefix': set(),
}
FEAT_TAG = {('Number', 'Sing'): 'singular', ('Number', 'Plur'): 'plural', ('Case', 'Nom'): 'nominative',
            ('Case', 'Gen'): 'genitive', ('Case', 'Dat'): 'dative', ('Case', 'Acc'): 'accusative',
            ('Case', 'Ins'): 'instrumental', ('Case', 'Loc'): 'locative', ('Person', '1'): 'first-person',
            ('Person', '2'): 'second-person', ('Person', '3'): 'third-person', ('Tense', 'Past'): 'past',
            ('Tense', 'Pres'): 'present', ('Gender', 'Masc'): 'masculine', ('Gender', 'Fem'): 'feminine',
            ('Gender', 'Neut'): 'neuter', ('Degree', 'Cmp'): 'comparative', ('Degree', 'Sup'): 'superlative'}


def fold(word, lang):
    text = unicodedata.normalize('NFD', word)
    text = ''.join(ch for ch in text if ch not in '́̀')
    text = unicodedata.normalize('NFC', text)
    return text.replace('ё', 'е').replace('Ё', 'Е') if lang == 'ru' else text


class Lexicon:
    def __init__(self, path, lang):
        self.lang = lang
        self.db = sqlite3.connect(path)
        self.meta = dict(self.db.execute('select key, value from meta'))
        variant = set(self.db.execute('select word, pos from entry where spelling_of is not null'))
        plain = set(self.db.execute('select word, pos from entry where spelling_of is null and form_of is null'))
        self.plain_words = {word for word, pos in plain if pos != 'name'}
        self.variants = variant - plain  # (word, part of speech) filed only as another word's spelling
        self.cache = {}

    def rows(self, form):
        key = fold(form, self.lang)
        if key not in self.cache:
            self.cache[key] = [(lemma, pos, json.loads(tags)) for lemma, pos, tags in
                               self.db.execute('select lemma, pos, tags from form where key=?', (key,))]
        return self.cache[key]

    def old_spelling_of(self, form):
        """The lemma whose entry lists this form as its obsolete or archaic alternative (Maß lists Maaß), or None."""
        if form in self.plain_words:  # a word of its own (Unruhe, beside an archaic spelling of Unruh)
            return None
        for lemma, pos, tags in self.rows(form):
            if lemma != form and 'alternative' in tags and set(tags) & {'obsolete', 'archaic', 'dated'}:
                return lemma
        return None

    def standard(self, form, names=True):
        """A form Wiktionary gives under a lemma that is not itself a spelling variant, and lists as no old spelling."""
        if self.old_spelling_of(form):
            return False
        return any((lemma, pos) not in self.variants and not set(tags) & OLD_TAGS and (names or pos != 'name')
                   for lemma, pos, tags in self.rows(form))

    def spelling_of(self, form, pos=None):
        rows = self.db.execute('select spelling_of, spelling_tags from entry where key=? and spelling_of is not null'
                               + (' and pos=?' if pos else ''), (fold(form, self.lang),) + ((pos,) if pos else ())).fetchall()
        return (rows[0][0], json.loads(rows[0][1] or '[]')) if rows else (None, None)


# ---------------------------------------------------------------- German spelling

RULES = [  # id, pattern, replacement, description
    ('th', r'(?<=[A-Za-zÄÖÜäöü])h(?=\b)|(?<=[Tt])h', '', 'th → t (Thier → Tier, Muth → Mut)'),
    ('dt-t', r'dt', 't', 'dt → t (todt → tot)'),
    ('dt-d', r'dt(?=\w)', 'd', 'dt → d inside a word (tödtlich → tödlich)'),
    ('ie-i', r'ie(?=ng|b)', 'i', 'ie → i before ng, b (gieng → ging, giebt → gibt)'),
    ('niss', r'niss\b', 'nis', '-niss → -nis (Gedächtniss → Gedächtnis)'),
    ('ss-sz', r'ss', 'ß', 'ss → ß after a long vowel or diphthong (grosse → große)'),
    ('umlaut', r'\b(A|O|U)e', lambda m: {'A': 'Ä', 'O': 'Ö', 'U': 'Ü'}[m.group(1)], 'Ae, Oe, Ue → Ä, Ö, Ü'),
    ('ee-e', r'ee', 'e', 'ee → e (Kameel → Kamel)'),
    ('aa-a', r'aa', 'a', 'aa → a (Waare → Ware)'),
    ('oo-o', r'oo', 'o', 'oo → o (Loos → Los)'),
    ('c-k', r'C(?=[aou])|c(?=[aou])', lambda m: 'K' if m.group(0) == 'C' else 'k', 'c → k before a, o, u (Cultur → Kultur)'),
    ('c-z', r'C(?=[ei])|c(?=[ei])', lambda m: 'Z' if m.group(0) == 'C' else 'z', 'c → z before e, i (Citrone → Zitrone)'),
    ('ey-ei', r'ey', 'ei', 'ey → ei (seyn → sein)'),
    ('iren', r'iren\b', 'ieren', '-iren → -ieren (regiren → regieren)'),
]
RULE_TEXT = {rid: text for rid, _, _, text in RULES}


def rule_candidates(word, depth=3):
    """Spellings the rules give for a word, each with the rules it took, fewest rules first."""
    out = {}
    applicable = [r for r in RULES if re.search(r[1], word)]
    for n in range(1, min(depth, len(applicable)) + 1):
        for combo in combinations(applicable, n):
            if {'dt-t', 'dt-d'} <= {r[0] for r in combo} or {'c-k', 'c-z'} <= {r[0] for r in combo}:
                continue
            text = word
            for _, pattern, repl, _ in combo:
                text = re.sub(pattern, repl, text)
            if text != word and text not in out:
                out[text] = [r[0] for r in combo]
            # ss → ß applies to one occurrence at a time where a word has several.
            if any(r[0] == 'ss-sz' for r in combo) and word.count('ss') > 1:
                for m in re.finditer('ss', word):
                    single = word[:m.start()] + 'ß' + word[m.end():]
                    for _, pattern, repl, _ in combo:
                        if _ != 'ss-sz':
                            single = re.sub(pattern, repl, single)
                    out.setdefault(single, [r[0] for r in combo])
    return sorted(out.items(), key=lambda item: len(item[1]))


def levenshtein(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def small_edit(word, target):
    """An orthographic difference of one letter: an h or e written or dropped (wol, gehn), a doubled letter (sammt)."""
    if abs(len(word) - len(target)) != 1:
        return False
    long, short = (word, target) if len(word) > len(target) else (target, word)
    for i, ch in enumerate(long):
        if long[:i] + long[i + 1:] == short:
            return ch in 'hHeE' or (i > 0 and long[i - 1] == ch) or (i + 1 < len(long) and long[i + 1] == ch)
    return False


def compound_known(word, lex):
    for i in range(3, len(word) - 2):
        head, tail = word[:i], word[i:]
        for left in (head, head.rstrip('s') if head.endswith('s') and len(head) > 4 else None):
            if left and lex.standard(left, names=False) and (lex.standard(tail.capitalize(), names=False) or lex.standard(tail, names=False)):
                return True
    return False


def modern_spelling(word, lex):
    """Today's spelling of a word of the first editions, with its ground, or None to keep it."""
    if not re.search(r'[A-Za-zÄÖÜäöüß]', word) or lex.standard(word) or lex.standard(word.lower()):
        return None
    # A compound of words in today's spelling is today's word (Fluchthaus, halbgewandt, herumriss).
    if compound_known(word, lex):
        return None
    target, tags = lex.spelling_of(word)
    if not target and lex.old_spelling_of(word):
        target, tags = lex.old_spelling_of(word), ['alternative']
    if target and target != word:
        if target in dict(rule_candidates(word)) or small_edit(word, target):
            return {'modern': target, 'basis': 'wiktionary', 'entry': word, 'tags': tags}
    for candidate, rules in rule_candidates(word):
        if lex.standard(candidate, names=False) or (candidate[0].isupper() and lex.standard(candidate.lower(), names=False) and not word[0].isupper()):
            return {'modern': candidate, 'basis': 'rules', 'rules': rules, 'lexicon': candidate}
    for candidate, rules in rule_candidates(word):
        if compound_known(candidate, lex):
            return {'modern': candidate, 'basis': 'rules+compound', 'rules': rules}
    return None


WORD = re.compile(r"[^\W\d_]+(?:['’][^\W\d_]+)?")


def reading_text(text, lang, spelling, counts):
    """The unit as the analyzer reads it, with a map back to the source's code points.

    Returns (reading, starts, ends): reading[i] comes from source code points
    starts[i] .. ends[i] (exclusive)."""
    src = CP(text)
    out, starts, ends = [], [], []
    i = 0
    while i < len(src):
        ch = src[i]
        if lang == 'de' and ch == '¬':  # a word broken at a line end reads whole
            j = i + 1
            while j < len(src) and src[j].isspace():
                j += 1
            if out:
                ends[-1] = j
            i = j
            JOINS[0] += 1
            continue
        out.append(' ' if ch == '\n' else ch)
        starts.append(i)
        ends.append(i + 1)
        i += 1
    reading = ''.join(out)
    if lang != 'de' or spelling is None:
        return reading, starts, ends
    pieces, s2, e2, last = [], [], [], 0
    for m in WORD.finditer(reading):
        word = m.group(0)
        if word not in spelling:
            spelling[word] = modern_spelling(word, LEX['de'])
        entry = spelling[word]
        pieces.append(reading[last:m.start()]); s2 += starts[last:m.start()]; e2 += ends[last:m.start()]
        if entry:
            counts[word] += 1
            new = entry['modern']
            a, b = starts[m.start()], ends[m.end() - 1]
            for k in range(len(new)):
                s2.append(a if k == 0 else min(b - 1, a + k)); e2.append(b if k == len(new) - 1 else min(b, a + k + 1))
            pieces.append(new)
        else:
            pieces.append(word); s2 += starts[m.start():m.end()]; e2 += ends[m.start():m.end()]
        last = m.end()
    pieces.append(reading[last:]); s2 += starts[last:]; e2 += ends[last:]
    return ''.join(pieces), s2, e2


# ---------------------------------------------------------------- lemma

def feats_dict(feats):
    return dict(item.split('=', 1) for item in feats.split('|')) if feats else {}


OPEN = {'NOUN', 'PROPN', 'VERB', 'ADJ', 'ADV'}


def choose_lemma(form, upos, feats, model_lemma, lex):
    """The lemma of a word in context and how firmly it stands (a, l or m).

    The dictionary corrects the model only where the model's lemma is no word
    at all (geingen for gieng, Mitleide for Mitleiden); a real word the model
    chose in context stands."""
    if lex is None or upos in ('PUNCT', 'SYM', 'NUM', 'X'):
        return model_lemma, 'm'
    rows = lex.rows(form)
    if not rows and form[:1].isupper() and upos not in ('NOUN', 'PROPN'):
        rows = lex.rows(form[:1].lower() + form[1:])
    fitting = []
    for lemma, pos, tags in rows:
        if upos not in POS_UPOS.get(pos, set()):
            continue
        if (lemma, pos) in lex.variants:  # Thal → Tal: a lemma filed as an old spelling reads in today's spelling
            target = lex.spelling_of(lemma, pos)[0]
            if target and (target in dict(rule_candidates(lemma)) or levenshtein(lemma, target) <= 1):
                lemma = target
        fitting.append((fold(lemma, lex.lang) if lex.lang == 'ru' else lemma, pos, tags))
    if lex.lang == 'ru':
        model_lemma = fold(model_lemma, 'ru')
    lemmas = {lemma for lemma, _, _ in fitting}
    if model_lemma in lemmas:
        return model_lemma, 'a'
    if upos not in OPEN or not fitting:
        return model_lemma, 'm'
    known = [(l, p, t) for l, p, t in lex.rows(model_lemma)
             if fold(l, lex.lang) == fold(model_lemma, lex.lang) and upos in POS_UPOS.get(p, set()) and (l, p) not in lex.variants]
    if known:
        return model_lemma, 'm'
    if upos == 'NOUN' and form in lemmas | {l.capitalize() for l in lemmas}:
        return form, 'l'  # a nominalised infinitive or adjective is its own lemma (das Lachen)
    wanted = {FEAT_TAG[(k, v)] for k, v in feats_dict(feats).items() if (k, v) in FEAT_TAG}
    other = set(FEAT_TAG.values()) - wanted
    best = max(fitting, key=lambda row: (len(wanted & set(row[2])) - 0.5 * len(set(row[2]) & other), row[0] == form))
    return best[0], 'l'


# ---------------------------------------------------------------- run

LEX = {}
JOINS = [0]
DEBUG = Counter()


def chapter_digest(units):
    return 'sha256:' + hashlib.sha256(json.dumps([[u[0], u[2]] for u in units], ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()


def model_files(lang, procs):
    import stanza
    base = os.path.join(stanza.resources.common.DEFAULT_MODEL_DIR, lang)
    out = {}
    for proc in procs:
        for path in sorted(glob.glob(os.path.join(base, proc, '*.pt'))):
            out[f'{proc}/{os.path.basename(path)}'] = 'sha256:' + hashlib.sha256(open(path, 'rb').read()).hexdigest()
    return out


def finalize(home):
    """Bind the word layer to the witnesses it was made from and index where each chapter begins.

    Rewrites, from the chapter files on disk, each witness's digest and the
    manifest's chapter index: for every language, each chapter with its first
    unit in witness order, so that a reader finds a unit's chapter without
    opening every file."""
    manifest_path = os.path.join(home, 'tokens.json')
    manifest = json.load(open(manifest_path))
    index = defaultdict(list)
    for path in sorted(glob.glob(os.path.join(home, 'witnesses', '*.json'))):
        chapter = json.load(open(path))
        token_path = os.path.join(home, 'tokens', f"{chapter['chapter']}.json")
        if not os.path.exists(token_path):
            continue
        layer = json.load(open(token_path))
        if layer['entries'] and len(layer['entries'][0]) == 4:  # an earlier layer kept the tagset tag: fold it away
            keep, remap = {}, {}
            for i, (lemma, upos, _xpos, feats) in enumerate(layer['entries']):
                remap[i] = keep.setdefault((lemma, upos, feats), len(keep))
            layer['entries'] = [list(k) for k in keep]
            for block in layer['witnesses'].values():
                for rows in block['units'].values():
                    for row in rows:
                        row[2] = remap[row[2]]
        for lang, block in layer['witnesses'].items():
            units = chapter['units'][lang]
            block['digest'] = chapter_digest(units)
            if units:
                index[lang].append([chapter['chapter'], units[0][0]])
        with open(token_path, 'w') as fh:
            json.dump(layer, fh, ensure_ascii=False, separators=(',', ':'))
            fh.write('\n')
    order = {c['key']: i for i, c in enumerate(json.load(open(os.path.join(home, 'witnesses.json')))['chapters'])}
    manifest['index'] = {lang: sorted(rows, key=lambda r: order.get(r[0], 0)) for lang, rows in index.items()}
    manifest['chapters'] = len(index.get('de') or next(iter(index.values()), []))
    with open(manifest_path, 'w') as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=1)
        fh.write('\n')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('repo')
    ap.add_argument('bonsai')
    ap.add_argument('--lexicon-dir', required=True)
    ap.add_argument('--chapters', nargs='*')
    ap.add_argument('--languages', nargs='*', default=['de', 'ru', 'en'])
    ap.add_argument('--finalize-only', action='store_true')
    args = ap.parse_args()
    home = os.path.join(args.repo, 'bonsai', args.bonsai)
    if args.finalize_only:
        return finalize(home)
    import stanza
    witnesses = json.load(open(os.path.join(home, 'witnesses.json')))
    files = sorted(glob.glob(os.path.join(home, 'witnesses', '*.json')))
    chapters = [json.load(open(f)) for f in files]
    if args.chapters:
        chapters = [c for c in chapters if c['chapter'] in args.chapters]
    for lang in args.languages:
        path = os.path.join(args.lexicon_dir, f'{lang}.sqlite')
        LEX[lang] = Lexicon(path, lang) if os.path.exists(path) else None
    spelling_path = os.path.join(home, 'spelling.json')
    spelling = {}
    counts = Counter()
    layers = defaultdict(dict)
    receipt = {}
    # Every pipeline loads before any text is read, so a missing model stops the run at once.
    pipelines = {}
    for lang in args.languages:
        procs = ['tokenize', 'mwt', 'pos', 'lemma', 'depparse'] if lang in ('de', 'en') else ['tokenize', 'pos', 'lemma', 'depparse']
        pipelines[lang] = (procs, stanza.Pipeline(lang, processors=','.join(procs), use_gpu=False, verbose=False, download_method=None))
    for lang in args.languages:
        procs, nlp = pipelines[lang]
        jobs = []
        for chapter in chapters:
            for unit in chapter['units'].get(lang, []):
                reading, starts, ends = reading_text(unit[2], lang, spelling if lang == 'de' else None, counts)
                jobs.append((chapter['chapter'], unit, reading, starts, ends))
        t0 = time.time()
        docs = nlp.bulk_process([stanza.Document([], text=job[2]) for job in jobs])
        seconds = time.time() - t0
        basis = Counter()
        words = 0
        for (chap, unit, reading, starts, ends), doc in zip(jobs, docs):
            rows = []
            for sentence in doc.sentences:
                offset = len(rows)
                order = []
                for token in sentence.tokens:
                    a, b = token.start_char, token.end_char
                    s, e = starts[a], ends[b - 1]
                    for word in token.words:
                        order.append((word, s, e, token.text))
                ids = {w.id: offset + k for k, (w, _, _, _) in enumerate(order)}
                for word, s, e, surface in order:
                    src = ''.join(CP(unit[2])[s:e])
                    norm = surface if surface.replace(' ', '') != src.replace('¬', '').replace('\n', '').replace(' ', '') else None
                    lemma, how = choose_lemma(word.text, word.upos, word.feats or '', word.lemma or word.text, LEX.get(lang))
                    if how == 'l':
                        DEBUG[(word.text, word.lemma, lemma, word.upos)] += 1
                    basis[how] += 1
                    words += word.upos != 'PUNCT'
                    head = ids.get(word.head, -1) if word.head else -1
                    row = [s, e, lemma, word.upos, word.xpos or '', word.feats or '', head, word.deprel, how]
                    if norm is not None or word.text != surface:
                        row.append(word.text if word.text != surface else norm)
                    rows.append(row)
            layers[chap].setdefault(lang, {})[unit[0]] = rows
        receipt[lang] = {'units': len(jobs), 'words': words, 'seconds': round(seconds, 1), 'lemma_basis': dict(basis),
                         'stanza_models': model_files(lang, procs),
                         'lexicon': LEX[lang].meta if LEX.get(lang) else None}
        print(json.dumps({lang: receipt[lang]['lemma_basis'], 'seconds': round(seconds)}), flush=True)
    # tokens/<chapter>.json: a shared vocabulary per chapter keeps the rows short.
    os.makedirs(os.path.join(home, 'tokens'), exist_ok=True)
    for chapter in chapters:
        chap = chapter['chapter']
        vocab, vindex, out = [], {}, {}
        path_out = os.path.join(home, 'tokens', f'{chap}.json')
        if os.path.exists(path_out):  # keep the languages this run did not read
            earlier = json.load(open(path_out))
            for lang, block in earlier['witnesses'].items():
                if lang in layers[chap]:
                    continue
                entries = earlier['entries']
                layers[chap][lang] = {uid: [[r[0], r[1], *entries[r[2]][:2], '', entries[r[2]][-1], r[3], r[4], r[5], *r[6:]] for r in rows]
                                      for uid, rows in block['units'].items()}
        for lang, units in layers[chap].items():
            lang_units = {}
            for uid, rows in units.items():
                packed = []
                for s, e, lemma, upos, xpos, feats, head, deprel, how, *norm in rows:
                    key = (lemma, upos, feats)  # UD part of speech and features carry the grammar; tagset tags stay with the model
                    if key not in vindex:
                        vindex[key] = len(vocab)
                        vocab.append(list(key))
                    packed.append([s, e, vindex[key], head, deprel, how] + norm)
                lang_units[uid] = packed
            out[lang] = {'digest': chapter_digest(chapter['units'][lang]), 'units': lang_units}
        payload = {'schema': 'tos.bonsai.tokens.chapter.v1', 'chapter': chap,
                   'fields': ['start', 'end', 'entry', 'head', 'deprel', 'basis', 'reading'],
                   'entries': vocab, 'witnesses': out}
        with open(os.path.join(home, 'tokens', f'{chap}.json'), 'w') as fh:
            json.dump(payload, fh, ensure_ascii=False, separators=(',', ':'))
            fh.write('\n')
    if os.environ.get('ANNOTATE_DEBUG'):
        print(json.dumps([[*k, n] for k, n in DEBUG.most_common(80)], ensure_ascii=False))
    if 'de' in args.languages:
        forms = {w: {**e, 'count': counts[w]} for w, e in sorted(spelling.items()) if e and counts[w]}
        bases = Counter(e['basis'] for e in forms.values())
        doc = {
            'schema': 'tos.bonsai.spelling.v1', 'witness': 'de',
            'posture': {'ru': 'Машинное предложение стенда, непроверенное: как каждое историческое написание первых изданий пишется сегодня. Текст свидетеля не меняется; современное написание служит чтению и разбору.',
                        'en': "A machine proposal of the stand, unreviewed: how each historical spelling of the first editions is written today. The witness's text never changes; today's spelling serves reading and analysis."},
            'sources': {'wiktionary': {'label': 'English Wiktionary via kaikki.org (Wiktextract)', 'url': 'https://kaikki.org/dictionary/German/',
                                       'extract_sha256': LEX['de'].meta.get('extract_sha256') if LEX.get('de') else None,
                                       'licence': 'CC BY-SA 4.0', 'attribution': 'Wiktionary contributors'}},
            'rules': RULE_TEXT,
            'counts': {'forms': len(forms), 'occurrences': sum(f['count'] for f in forms.values()), 'line_end_joins': JOINS[0], 'basis': dict(bases)},
            'forms': forms}
        with open(spelling_path, 'w') as fh:
            json.dump(doc, fh, ensure_ascii=False, indent=1)
            fh.write('\n')
    manifest_path = os.path.join(home, 'tokens.json')
    if os.path.exists(manifest_path):  # a language this run did not read keeps its receipt
        for lang, item in json.load(open(manifest_path)).get('receipt', {}).items():
            receipt.setdefault(lang, item)
    manifest = {
        'schema': 'tos.bonsai.tokens.v1', 'work': witnesses.get('work'),
        'posture': {'ru': 'Машинный разбор стенда, непроверенный: каждое слово каждого свидетеля на его точном месте — лемма, часть речи, признаки и синтаксическая вершина в предложении. Расхождение модели и словаря отмечено.',
                    'en': 'A machine analysis of the stand, unreviewed: every word of every witness at its exact place, with lemma, part of speech, features and syntactic head in its sentence. Where model and dictionary disagree, it is marked.'},
        'offsets': 'Unicode code points of the unit text; a word broken at a line end spans the break',
        'basis': {'a': {'ru': 'лемма модели совпадает с леммой Викисловаря для этой формы и части речи', 'en': "the model's lemma is among Wiktionary's lemmas for this form and part of speech"},
                  'l': {'ru': 'лемма взята из Викисловаря: она подходит по части речи и признакам, лемма модели — нет', 'en': "Wiktionary's lemma fitting the part of speech and features replaces the model's"},
                  'm': {'ru': 'Викисловарь формы не знает; лемма модели', 'en': "Wiktionary does not know the form; the model's lemma"}},
        'analyzer': {'tool': 'bonsai/review/philology/annotate.py', 'stanza': stanza.__version__, 'universal_dependencies': 'UD v2 part-of-speech, features and relations'},
        'spelling': 'spelling.json' if 'de' in args.languages else None,
        'chapters': len(chapters), 'receipt': receipt}
    with open(os.path.join(home, 'tokens.json'), 'w') as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=1)
        fh.write('\n')
    finalize(home)


if __name__ == '__main__':
    sys.exit(main())
