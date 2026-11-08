#!/usr/bin/env python3
"""Gather the history of every lemma of a bonsai's witnesses.

    python3 histories.py told <dewiktionary.jsonl.gz> <ruwiktionary.jsonl.gz> --out <dir>
    python3 histories.py attest <repo> <bonsai> --cache <file.jsonl>
    python3 histories.py build <repo> <bonsai> --lexicon-dir <dir> --told-dir <dir> --attest <file.jsonl>

The origin of a word (`origins.py`) is its descent; its history is what the
dictionaries tell of it and where the German written before and beside the
book uses it. `build` writes `histories.json` and
`histories/<language>-<shard>.json`; the record of a lemma holds:

- `told`: the etymology as each Wiktionary edition tells it, in the edition's
  language (`en` English Wiktionary, `de` German, `ru` Russian), its first
  sentences without the lists of kindred forms, with the entry it is from and
  the dictionary the edition names as its basis;
- `attested` (the original's words): the word in the Deutsches Textarchiv
  (DTA), the corpus of German texts from the 15th to the early 20th century:
  its uses per quarter century outside Nietzsche's works, the earliest of them
  by up to three authors with the sentence and the page, and Nietzsche's works
  in the corpus that use it;
- `book`: how often the book uses it, per part.

`told` reads the Wiktextract extracts of the German and Russian Wiktionary
(kaikki.org) into small indexes; the English edition's text is already in the
lexicon of `lexicon.py`. `attest` asks the DTA's public corpus search (D*,
ddc.dwds.de) and keeps every answer in a cache file, so that `build` runs
offline and a second `attest` asks only what the cache lacks. Wiktionary's text
and the DTA's texts are available under CC BY-SA 4.0.
"""
import argparse, datetime, glob, gzip, hashlib, json, os, re, sqlite3, sys, threading, time, urllib.parse, urllib.request
from concurrent.futures import ThreadPoolExecutor
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from origins import CONTENT, POS_UPOS, SHARDS, fold, shard_of  # noqa: E402

DTA = 'https://ddc.dwds.de/dstar/dta/dstar.perl'
DTA_BOOK = 'https://www.deutschestextarchiv.de/book/view/{basename}/?p={page}&hl={hl}'
AGENT = 'TreeOfSophia-philology/1 (+https://treeofsophia.com)'
STEP = 25
NIETZSCHE = '#has[author,/Nietzsche/]'
EDITIONS = {'de': {'label': 'German Wiktionary', 'site': 'https://de.wiktionary.org/wiki/', 'languages': ('de',)},
            'ru': {'label': 'Russian Wiktionary', 'site': 'https://ru.wiktionary.org/wiki/', 'languages': ('ru', 'de')}}
ENGLISH_SITE = 'https://en.wiktionary.org/wiki/'
WIKT_LANGUAGE = {'de': 'German', 'ru': 'Russian'}


# ---------------------------------------------------------------- the telling

LATIN_ABBREVIATIONS = {'vgl', 'lat', 'gr', 'griech', 'ahd', 'mhd', 'got', 'engl', 'frz', 'ital', 'span', 'ca', 'cf', 'c',
                       'v', 's', 'bzw', 'etc', 'jh', 'jhd', 'jahrh', 'z', 'b', 'u', 'a', 'e', 'g', 'i', 'lit', 'ff', 'f'}


def ends_sentence(before):
    """Whether the stop that ends `before` ends a sentence: an abbreviation (лат., др.-греч., vgl.) or an ordinal (8.) does not."""
    word = re.search(r'(\S+)$', before[:-1])
    if not word:
        return True
    word = word.group(1).lstrip('(„"«')
    if word.isdigit() or re.search(r'\.-?\w{1,7}$', word):
        return False
    if re.fullmatch(r'[а-яёА-ЯЁ][а-яё]{0,6}', word):
        return False
    return word.lower() not in LATIN_ABBREVIATIONS


def sentences(text):
    """Split prose at sentence ends."""
    out, start = [], 0
    for match in re.finditer(r'[.!?…](?=\s+(?:[A-ZÄÖÜА-ЯЁ„“"«(]|$))', text):
        before = text[start:match.end()]
        if not ends_sentence(before):
            continue
        out.append(before.strip())
        start = match.end()
    rest = text[start:].strip()
    if rest:
        out.append(rest)
    return out


KIN_LIST = re.compile(r',?\s*от которого в числе прочего произош(?:ли|ло|ёл):.*?\.(?=\s+[А-ЯЁ]|\s*$)', re.S)
DROP_RU = re.compile(r'^(?:Родственн|Ср\.|Сравнивают|Неубедительн|Другая ступень|Продолжает|См\.|Любопытно|Считают|Дальнейш|Сюда же)')
DROP_EN = re.compile(r'^(?:Cognate|Compare|Doublet|Related|See|More at|Akin|Equivalent|Displaced|From .{1,60} (?:is|are) borrowed)')
DROP_DE = re.compile(r'^(?:Vergleiche|Siehe|Verwandt|Kognat)')
BASIS = re.compile(r'\s*Использованы данные\s+(.+?)(?:\.|;)?\s*(?:[Сс]м\.\s*Список литературы\.?)?\s*(?=Использованы данные|$)')
TREE = re.compile(r'^Etymology tree\b.*?(?=\b(?:From|Inherited from|Borrowed from|Calque of|Derived from|Learned borrowing)\b)')


def tidy(text):
    text = re.sub(r'\s*\^\(→ [^)]*\)|\^☆', '', text)      # the German edition's language links
    text = re.sub(r'^\s*\[[\d,\s–-]+\]\s*', '', text)          # its sense numbers
    text = re.sub(r"''+", '', text)
    text = re.sub(r'\s+', ' ', text).strip()
    text = TREE.sub('', text)
    text = re.sub(r'^\*(?=[A-ZÄÖÜ])', '', text)
    text = re.sub(r';\s*(?:cognate|compare) with [^.;]*', '', text, flags=re.I)
    return text


def told_text(texts, edition, limit=420):
    """The first sentences of an edition's etymology: no kindred lists, no unknown forms, the basis apart."""
    basis, parts = None, []
    for raw in texts or []:
        text = tidy(raw)
        if not text or text.startswith(('„', '"', '“')) or '??' in text or 'Шаблон:' in text or ':Template' in text:
            continue
        if edition == 'ru':
            found = list(BASIS.finditer(text))
            if found:
                basis = (basis or []) + [item.group(1).strip().rstrip('.;') for item in found]
                text = text[:found[0].start()].strip()
            text = re.sub(r'\s*См\.\s*Список литературы\.?', '', text)
            text = KIN_LIST.sub('.', text)
            text = re.sub(r',?\s*(?:далее\s+)?из\s+праслав\.\s+\*\?[^.]*', '', text)
        if edition == 'de':
            text = re.sub(r'^etymologisch:\s*', '', text)
            text = text[:1].upper() + text[1:]
        if text and text[-1] not in '.!?…»)':
            text += '.'
        parts.append(text)
    drop = {'ru': DROP_RU, 'en': DROP_EN, 'de': DROP_DE}[edition]
    kept, size = [], 0
    for sentence in sentences(' '.join(parts)):
        sentence = re.sub(r'\.\.+$', '.', sentence)
        if drop.match(sentence):
            if kept:
                break
            continue
        if kept and size + len(sentence) > limit:
            break
        kept.append(sentence)
        size += len(sentence) + 1
        if len(kept) == 3:
            break
    text = ' '.join(kept).strip()
    if text and text[-1] not in '.!?…»)':
        text += '.'
    return (text or None), basis


def told_index(args):
    """Index the etymologies of the German and Russian Wiktionary extracts for the languages a stand reads."""
    os.makedirs(args.out, exist_ok=True)
    for edition, path in (('de', args.de), ('ru', args.ru)):
        out = os.path.join(args.out, f'told-{edition}.sqlite')
        if os.path.exists(out):
            os.remove(out)
        db = sqlite3.connect(out)
        db.executescript('create table meta(key text primary key, value text);'
                         'create table told(key text, word text, lang text, pos text, texts text);'
                         'create index told_key on told(key, lang);')
        digest, kept = hashlib.sha256(), 0
        languages = EDITIONS[edition]['languages']
        with open(path, 'rb') as raw:
            for chunk in iter(lambda: raw.read(1 << 20), b''):
                digest.update(chunk)
        with gzip.open(path, 'rt', encoding='utf-8') as fh:
            for line in fh:
                if '"etymology_text' not in line:
                    continue
                record = json.loads(line)
                lang = record.get('lang_code')
                texts = record.get('etymology_texts') or ([record['etymology_text']] if record.get('etymology_text') else [])
                if lang not in languages or not texts or not record.get('word'):
                    continue
                db.execute('insert into told values (?,?,?,?,?)',
                           (fold(record['word'], lang), record['word'], lang, record.get('pos'), json.dumps(texts, ensure_ascii=False)))
                kept += 1
        db.executemany('insert into meta values (?,?)', [('extract', os.path.basename(path)), ('extract_sha256', digest.hexdigest()),
                                                         ('edition', edition), ('entries', str(kept))])
        db.commit()
        print(json.dumps({'edition': edition, 'entries': kept}))


# ---------------------------------------------------------------- the book

def book_lemmas(home):
    """Every lemma of every witness: how often the book uses it, per part, and its most frequent part of speech."""
    witnesses = json.load(open(os.path.join(home, 'witnesses.json')))
    table = defaultdict(lambda: {'n': 0, 'parts': Counter(), 'upos': Counter()})
    for chapter in witnesses['chapters']:
        path = os.path.join(home, 'tokens', f"{chapter['key']}.json")
        if not os.path.exists(path):
            continue
        layer = json.load(open(path))
        part = chapter['key'].split('.')[0]
        entries = layer['entries']
        for lang, block in layer['witnesses'].items():
            for rows in block['units'].values():
                for row in rows:
                    lemma, upos = entries[row[2]][0], entries[row[2]][1]
                    if upos in ('PUNCT', 'NUM', 'SYM', 'X'):
                        continue
                    record = table[(lang, lemma)]
                    record['n'] += 1
                    record['parts'][part] += 1
                    record['upos'][upos] += 1
    return table


def original_of(home):
    witnesses = json.load(open(os.path.join(home, 'witnesses.json')))
    return witnesses.get('pivot') if isinstance(witnesses.get('pivot'), str) else 'de'


def asked_lemmas(home):
    """The original's content words, each with the spellings the DTA's lemmatizer may give it (Antheil, Anteil)."""
    spelling = json.load(open(os.path.join(home, 'spelling.json'))).get('forms', {})
    original = original_of(home)
    out = {}
    for (lang, lemma), record in book_lemmas(home).items():
        if lang != original or record['upos'].most_common(1)[0][0] not in CONTENT - {'PROPN'}:
            continue
        names = {lemma}
        modern = spelling.get(lemma, {}).get('modern')
        if modern:
            names.add(modern)
        out[lemma] = sorted(names)
    return out


# ---------------------------------------------------------------- the DTA

class Corpus:
    """The DTA's corpus search, every answer kept in a cache file by its query."""

    def __init__(self, cache):
        self.path = cache
        self.answers = {}
        if os.path.exists(cache):
            with open(cache, encoding='utf-8') as fh:
                for line in fh:
                    if line.strip():
                        item = json.loads(line)
                        self.answers[(item['q'], item['limit'])] = item
        self.asked = 0
        self.lock = threading.Lock()

    def ask(self, query, limit, offline=False):
        key = (query, limit)
        if key in self.answers or offline:
            return self.answers.get(key)
        url = f"{DTA}?{urllib.parse.urlencode({'q': query, 'fmt': 'json', 'limit': limit})}"
        for attempt in range(4):
            try:
                request = urllib.request.Request(url, headers={'User-Agent': AGENT})
                with urllib.request.urlopen(request, timeout=120) as response:
                    data = json.load(response)
                break
            except Exception as error:  # a busy server answers again a little later
                if attempt == 3:
                    raise
                print(f'retry {attempt + 1}: {error}', file=sys.stderr)
                time.sleep(5 * (attempt + 1))
        item = {'q': query, 'limit': limit, 'at': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')}
        if 'counts_' in data:
            item['counts'] = data['counts_']
        item['hits'] = [compact_hit(hit) for hit in data.get('hits_', [])]
        item['n'] = data.get('nhits_')
        with self.lock:
            self.answers[key] = item
            with open(self.path, 'a', encoding='utf-8') as fh:
                fh.write(json.dumps(item, ensure_ascii=False) + '\n')
            self.asked += 1
        return item


def compact_hit(hit):
    """A hit as the history keeps it: the work, the page, the sentence as the work spells it, the matched words."""
    meta = hit.get('meta_', {})
    tokens = [token for part in hit.get('ctx_', []) for token in part]
    text, spans, page, mark = '', [], None, None
    for token in tokens:
        word = token.get('w', '')
        if text and token.get('ws', '1') != '0':
            text += ' '
        start = len(text)
        text += word
        if token.get('hl_'):
            spans.append([start, len(text)])
            page = page or token.get('page')
            mark = mark or token.get('u')
    return {'date': meta.get('date_'), 'author': meta.get('author'), 'title': meta.get('title'), 'bibl': meta.get('bibl'),
            'basename': meta.get('basename'), 'page': page, 'mark': mark, 'availability': meta.get('availability'),
            'text': text, 'spans': spans}


def quote(name):
    return "'" + name.replace('\\', '\\\\').replace("'", "\\'") + "'"


def lemma_set(names):
    return '$l=@{' + ','.join(quote(name) for name in names) + '}'


def count_queries(groups):
    """The batched questions: uses per lemma and quarter century outside Nietzsche, and Nietzsche's works with it."""
    for group in groups:
        names = sorted({name for _, aliases in group for name in aliases})
        yield 'periods', f'count({lemma_set(names)} !{NIETZSCHE}) #by[$l,date/{STEP}]'
        yield 'nietzsche', f'count({lemma_set(names)} {NIETZSCHE}) #by[$l,date,title]'


def early_query(names):
    return f'{lemma_set(names)} !{NIETZSCHE} #asc_date'


def corpus_size_query():
    """Nouns per quarter century: the measure a lemma's uses are read against."""
    return f'count($p=NN) #by[date/{STEP}]'


def batches(items, size):
    items = list(items)
    for index in range(0, len(items), size):
        yield items[index:index + size]


def attest(args):
    home = os.path.join(args.repo, 'bonsai', args.bonsai)
    corpus = Corpus(args.cache)
    asked = asked_lemmas(home)
    corpus.ask(corpus_size_query(), 1000)
    for kind, query in count_queries(batches(sorted(asked.items()), args.batch)):
        corpus.ask(query, 100000)
    used = used_lemmas(corpus, asked, offline=True)
    todo = [lemma for lemma in sorted(asked) if used.get(lemma)]

    def early(lemma):
        corpus.ask(early_query(asked[lemma]), args.early)
        if corpus.asked and corpus.asked % 100 == 0:
            print(f'{corpus.asked} questions asked of {len(todo)} lemmas', file=sys.stderr)

    # A few questions at a time: the corpus search is a shared public service.
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        list(pool.map(early, todo))
    print(json.dumps({'lemmas': len(asked), 'used': len(todo), 'asked': corpus.asked}))


def used_lemmas(corpus, asked, offline=False):
    """Per lemma, its uses per quarter century outside Nietzsche, from the batched answers in the cache."""
    by_name = defaultdict(set)
    for lemma, names in asked.items():
        for name in names:
            by_name[name].add(lemma)
    periods = defaultdict(Counter)
    for kind, query in count_queries(batches(sorted(asked.items()), BATCH)):
        if kind != 'periods':
            continue
        answer = corpus.ask(query, 100000, offline=offline)
        for n, name, start in (answer or {}).get('counts', []):
            for lemma in by_name.get(name, ()):
                periods[lemma][int(start)] += int(n)
    return periods


def nietzsche_works(corpus, asked):
    by_name = defaultdict(set)
    for lemma, names in asked.items():
        for name in names:
            by_name[name].add(lemma)
    works = defaultdict(dict)
    for kind, query in count_queries(batches(sorted(asked.items()), BATCH)):
        if kind != 'nietzsche':
            continue
        for n, name, date, title in (corpus.ask(query, 100000, offline=True) or {}).get('counts', []):
            year = int(str(date)[:4])
            title = short_title(title)
            for lemma in by_name.get(name, ()):
                works[lemma][title] = min(year, works[lemma].get(title, year))
    return {lemma: sorted([[year, title] for title, year in table.items()]) for lemma, table in works.items()}


def short_title(title, limit=70):
    """A work's title to its first colon or full stop (D. Katzenbergers keeps its initial), cut at a word within `limit`."""
    title = (title or '').strip()
    for match in re.finditer(r'\s*(?:[:;]|(?<=\w{2})\.)\s+', title):
        title = title[:match.start()].strip()
        break
    if len(title) > limit:
        title = title[:limit].rsplit(' ', 1)[0].rstrip(',/;') + ' …'
    return title


def author_name(author):
    """`Herder, Johann Gottfried von (#118549553)` reads Johann Gottfried von Herder; pseudonyms in brackets and N. N. drop."""
    name = (author or '').split(';')[0]
    name = re.sub(r'\s*\([^)]*\)?', '', name).strip()
    if ',' in name:
        last, first = [part.strip() for part in name.split(',', 1)]
        name = f'{first} {last}'.strip()
    return None if re.fullmatch(r'N\.?\s*N\.?|Nn|[Aa]nonym\w*', name) else (name or None)


def window(hit, words=14):
    """The sentence around the first matched word, at most `words` words to each side."""
    text, spans = hit['text'], hit['spans']
    if not spans:
        return None
    start, end = spans[0]
    left = text[:start].split(' ')
    right = text[end:].split(' ')
    cut_left = len(left) > words + 1
    cut_right = len(right) > words + 1
    left = left[-(words + 1):] if cut_left else left
    right = right[:words + 1] if cut_right else right
    before = ' '.join(left)
    after = ' '.join(right)
    shown = ('… ' if cut_left else '') + before
    begin = len(shown)
    shown += text[start:end]
    finish = len(shown)
    shown += after + (' …' if cut_right else '')
    return shown.strip(), [begin - (len(shown) - len(shown.lstrip())), finish - (len(shown) - len(shown.lstrip()))]


def early_uses(answer, works=3):
    """The earliest uses by up to `works` authors, each author once, from openly licensed texts."""
    out, seen = [], set()
    for hit in (answer or {}).get('hits', []):
        if 'Creative Commons' not in (hit.get('availability') or '') or not hit.get('basename'):
            continue
        voice = author_name(hit.get('author')) or hit['basename']
        if voice in seen:
            continue
        shown = window(hit)
        if not shown:
            continue
        seen.add(voice)
        text, span = shown
        out.append([int(str(hit['date'])[:4]), author_name(hit.get('author')), short_title(hit.get('title')), text, span,
                    hit['basename'], int(hit['page']) if str(hit.get('page') or '').isdigit() else None, hit.get('mark')])
        if len(out) == works:
            break
    return out


BATCH = 80


# ---------------------------------------------------------------- the module

class Told:
    def __init__(self, lexicon_dir, told_dir):
        self.english = {lang: sqlite3.connect(os.path.join(lexicon_dir, f'{lang}.sqlite'))
                        for lang in ('de', 'ru') if os.path.exists(os.path.join(lexicon_dir, f'{lang}.sqlite'))}
        self.native = {edition: sqlite3.connect(os.path.join(told_dir, f'told-{edition}.sqlite'))
                       for edition in EDITIONS if os.path.exists(os.path.join(told_dir, f'told-{edition}.sqlite'))}

    def meta(self):
        out = {}
        for lang, db in self.english.items():
            out[f'en/{lang}'] = dict(db.execute('select key, value from meta'))
        for edition, db in self.native.items():
            out[edition] = dict(db.execute('select key, value from meta'))
        return out

    @staticmethod
    def pick(rows, upos):
        """The entry whose part of speech fits the word's, else the first that tells anything."""
        for fits in (True, False):
            for row in rows:
                if (upos in POS_UPOS.get(row[1], set())) == fits:
                    yield row

    def tell(self, lang, lemma, upos):
        out = {}
        keys = [fold(lemma, lang)]
        if lemma[:1].isupper():
            keys.append(fold(lemma[:1].lower() + lemma[1:], lang))
        db = self.english.get(lang)
        if db:
            for key in keys:
                rows = db.execute('select word, pos, etymology from entry where key=? and form_of is null and spelling_of is null'
                                  ' and etymology is not null and etymology != ""', (key,)).fetchall()
                for word, pos, etymology in self.pick(rows, upos):
                    text, basis = told_text([etymology], 'en')
                    if text:
                        out['en'] = {'text': text, 'entry': word}
                        break
                if 'en' in out:
                    break
        for edition, db in self.native.items():
            for key in keys:
                rows = db.execute('select word, pos, texts from told where key=? and lang=?', (key, lang)).fetchall()
                for word, pos, texts in self.pick(rows, upos):
                    text, basis = told_text(json.loads(texts), edition)
                    if text:
                        out[edition] = {'text': text, 'entry': word, **({'basis': basis} if basis else {})}
                        break
                if edition in out:
                    break
        return out


def build(args):
    home = os.path.join(args.repo, 'bonsai', args.bonsai)
    original = original_of(home)
    told = Told(args.lexicon_dir, args.told_dir)
    corpus = Corpus(args.attest) if args.attest else None
    asked = asked_lemmas(home) if corpus else {}
    periods = used_lemmas(corpus, asked, offline=True) if corpus else {}
    works = nietzsche_works(corpus, asked) if corpus else {}
    size = {int(start): int(n) for n, start in (corpus.ask(corpus_size_query(), 1000, offline=True) or {}).get('counts', [])} if corpus else {}
    span = sorted(size) or [0]
    counts = Counter()
    shards = defaultdict(lambda: defaultdict(dict))
    for (lang, lemma), record in sorted(book_lemmas(home).items()):
        if lang not in ('de', 'ru'):
            continue
        upos = record['upos'].most_common(1)[0][0]
        item = {'book': {'n': record['n'], 'parts': dict(sorted(record['parts'].items()))}}
        if upos in CONTENT:
            tale = told.tell(lang, lemma, upos)
            if tale:
                item['told'] = tale
                counts[f'{lang}_told'] += 1
        if lang == original and periods.get(lemma):
            used = periods[lemma]
            first_period = min(used)
            answer = corpus.ask(early_query(asked[lemma]), args.early, offline=True)
            early = early_uses(answer)
            attested = {'from': span[0], 'n': [used.get(start, 0) for start in span]}
            dates = [int(str(hit['date'])[:4]) for hit in (answer or {}).get('hits', []) if str(hit.get('date') or '')[:4].isdigit()]
            if dates:
                attested['first'] = min(dates)
            else:
                attested['period'] = [first_period, first_period + STEP - 1]
            if early:
                attested['early'] = early
            if works.get(lemma):
                attested['nietzsche'] = works[lemma]
            item['attested'] = attested
            counts[f'{lang}_attested'] += 1
        elif lang == original and works.get(lemma):
            item['attested'] = {'from': span[0], 'n': [0] * len(span), 'nietzsche': works[lemma]}
            counts[f'{lang}_attested_nietzsche_only'] += 1
        shards[lang][shard_of(lemma)][lemma] = item
        counts[f'{lang}_lemmas'] += 1
    os.makedirs(os.path.join(home, 'histories'), exist_ok=True)
    for old in glob.glob(os.path.join(home, 'histories', '*.json')):
        os.remove(old)
    for lang, table in shards.items():
        for shard, items in table.items():
            with open(os.path.join(home, 'histories', f'{lang}-{shard:02d}.json'), 'w', encoding='utf-8') as fh:
                json.dump({'schema': 'tos.bonsai.histories.shard.v1', 'language': lang, 'shard': shard,
                           'lemmas': dict(sorted(items.items()))}, fh, ensure_ascii=False, separators=(',', ':'))
                fh.write('\n')
    meta = told.meta()
    witnesses = json.load(open(os.path.join(home, 'witnesses.json')))
    asked_at = sorted(item['at'] for item in corpus.answers.values()) if corpus and corpus.answers else []
    manifest = {
        'schema': 'tos.bonsai.histories.v1', 'work': witnesses.get('work'), 'original': original,
        'posture': {
            'ru': 'История каждого слова свидетелей: что рассказывают о нём словари Викисловаря, где его употребляют немецкие тексты от XV до начала XX века (Немецкий текстовый архив, DTA) и сколько раз его говорит книга.',
            'en': "The history of each word of the witnesses: what Wiktionary's dictionaries tell of it, where the German texts of the 15th to the early 20th century use it (the German Text Archive, DTA), and how often the book says it."},
        'shards': {'count': SHARDS, 'key': 'FNV-1a 32-bit over the UTF-8 bytes of the lemma, modulo the count',
                   'path': 'histories/<language>-<shard, two digits>.json'},
        'told': {
            'method': "the etymology of the entry whose part of speech fits the word, its first sentences (at most three, about 420 characters) without the lists of kindred forms; the Russian edition's named basis kept apart",
            'editions': {
                'en': {'label': 'English Wiktionary via kaikki.org (Wiktextract)', 'site': ENGLISH_SITE,
                       'extracts': {lang: value.get('extract_sha256') for key, value in meta.items() if key.startswith('en/') for lang in [key[3:]]}},
                **{edition: {'label': f"{EDITIONS[edition]['label']} via kaikki.org (Wiktextract)", 'site': EDITIONS[edition]['site'],
                             'extract': meta[edition].get('extract'), 'extract_sha256': meta[edition].get('extract_sha256'),
                             'url': f'https://kaikki.org/{edition}wiktionary/rawdata.html'}
                   for edition in EDITIONS if edition in meta}},
            'licence': 'CC BY-SA 4.0', 'licence_url': 'https://creativecommons.org/licenses/by-sa/4.0/', 'attribution': 'Wiktionary contributors'},
        'attested': {
            'corpus': 'Deutsches Textarchiv (DTA), Berlin-Brandenburgische Akademie der Wissenschaften',
            'url': 'https://www.deutschestextarchiv.de/', 'search': DTA, 'book': DTA_BOOK,
            'asked': {'from': asked_at[0], 'to': asked_at[-1]} if asked_at else None,
            'queries': {'periods': f'count($l=@{{…}} !{NIETZSCHE}) #by[$l,date/{STEP}]',
                        'nietzsche': f'count($l=@{{…}} {NIETZSCHE}) #by[$l,date,title]',
                        'early': f'$l=@{{…}} !{NIETZSCHE} #asc_date', 'size': corpus_size_query()},
            'periods': {'from': span[0], 'step': STEP, 'count': len(span)},
            'nouns': [size.get(start, 0) for start in span],
            'first': 'the year of the earliest use outside Nietzsche; `period`, the quarter century of the earliest uses, where the hits are not at hand',
            'early': f'[year, author, title, sentence, [start, end] of the word in it, DTA basename, page, the word as the work spells it]; the earliest uses among the first {args.early} hits by date, one per author, texts under a Creative Commons licence only',
            'posture': "uses of the lemma as the DTA's lemmatizer reads it, outside Nietzsche's works; `nouns` holds the corpus's nouns per period, the measure a lemma's uses are read against",
            'licence': 'CC BY-SA 4.0 (the texts quoted)', 'licence_url': 'https://creativecommons.org/licenses/by-sa/4.0/'},
        'licence': 'CC BY-SA 4.0', 'counts': dict(sorted(counts.items()))}
    with open(os.path.join(home, 'histories.json'), 'w', encoding='utf-8') as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=1)
        fh.write('\n')
    print(json.dumps(manifest['counts']))


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='command', required=True)
    one = sub.add_parser('told')
    one.add_argument('de')
    one.add_argument('ru')
    one.add_argument('--out', required=True)
    two = sub.add_parser('attest')
    two.add_argument('repo')
    two.add_argument('bonsai')
    two.add_argument('--cache', required=True)
    two.add_argument('--batch', type=int, default=BATCH)
    two.add_argument('--early', type=int, default=12)
    two.add_argument('--workers', type=int, default=3)
    three = sub.add_parser('build')
    three.add_argument('repo')
    three.add_argument('bonsai')
    three.add_argument('--lexicon-dir', required=True)
    three.add_argument('--told-dir', required=True)
    three.add_argument('--attest')
    three.add_argument('--early', type=int, default=12)
    args = ap.parse_args()
    {'told': told_index, 'attest': attest, 'build': build}[args.command](args)


if __name__ == '__main__':
    sys.exit(main())
