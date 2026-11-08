#!/usr/bin/env python3
"""The short story of a word, told from its gathered sources only, and the check it passes.

    python3 stories.py packets <repo> <bonsai> --out <dir> [--terms id,id,…]
    python3 stories.py check <dir>
    python3 stories.py bundle <dir> <repo> <bonsai>

`packets` gathers, for each single-word term of a bonsai, everything the stand
already holds of its word into one packet, every piece with its own id: the
etymology as each Wiktionary edition tells it (histories), its descent and
parts (origins), its uses in the Deutsches Textarchiv and Nietzsche's works
there (histories), how often and where the book says it, with the sentence in
the original and in the translations shown beside it (witnesses and their
alignment), the translators' words for it (renderings) and the Tree's own word
and gloss (terms). It writes `<dir>/packets/<term>.json`.

A writer reads one packet and writes `<dir>/stories/<term>.json`: the story in
Russian and in English, each sentence with the ids of the pieces it rests on,
and every proper name it writes in a form other than the source's own with that
form beside it. `check` reads each story against its packet:

- every sentence names pieces of its packet;
- every number of a sentence stands, as a whole number, in the pieces it names;
- every quotation stands word for word in the pieces it names;
- every name a sentence writes stands in those pieces, among the names the story
  declares with their source form, or among the names the stand reads by itself
  (languages, Wiktionary, the archive, Nietzsche, Zarathustra, the book's
  chapters, the translators); a sentence's first word passes as an ordinary
  word when the witnesses, the packets or the stories write it in lowercase;
- a story keeps its length and its language to affirmation.

It writes `<dir>/check.json` and prints a summary. The story is the writer's
proposal; the check and a reviewing model stand between it and the stand.

`bundle` writes the bonsai's `stories.json` (`tos.bonsai.stories.v1`) only
when every packet has a story that passes the check and stands on the packet
this checkout gives; otherwise it names the terms and writes nothing. The layer
is keyed by the original's lemma: each
story in both languages with its sentences' pieces, and every piece it cites in
the form the stand shows it (a place of the book with its unit, so that the
reader opens it).
"""
import argparse, glob, hashlib, json, os, re, sys, unicodedata
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from origins import shard_of  # noqa: E402

PARTS = {'I': 'I', 'II': 'II', 'III': 'III', 'IV': 'IV'}
EDITION_URL = {'en': 'https://en.wiktionary.org/wiki/{entry}#{section}', 'de': 'https://de.wiktionary.org/wiki/{entry}',
               'ru': 'https://ru.wiktionary.org/wiki/{entry}'}
SECTION = {'de': 'German', 'ru': 'Russian'}
DTA_BOOK = 'https://www.deutschestextarchiv.de/book/view/{basename}/?p={page}'
LIMIT_WORDS = 130


def read(path):
    with open(path, encoding='utf-8') as fh:
        return json.load(fh)


def language_names(repo):
    """Wiktionary's language codes with their Russian and English names, as the stand shows them."""
    text = open(os.path.join(repo, 'access/web/src/corpus-reader/language-names.mjs'), encoding='utf-8').read()
    return {code: {'ru': ru, 'en': en} for code, ru, en in re.findall(r"\['([^']+)','([^']+)','([^']+)'\]", text)}


def clean(text):
    return re.sub(r'\s+', ' ', re.sub(r'¬\s*', '', text or '')).strip()


def record(home, folder, language, lemma):
    path = os.path.join(home, folder, f'{language}-{shard_of(lemma):02d}.json')
    return read(path)['lemmas'].get(lemma) if os.path.exists(path) else None


class Book:
    """The book's chapters with their witnesses and the sentence alignment between them."""

    def __init__(self, home):
        self.home = home
        self.witnesses = read(os.path.join(home, 'witnesses.json'))
        self.order = [chapter['key'] for chapter in self.witnesses['chapters']]
        self.titles = {chapter['key']: chapter['titles'] for chapter in self.witnesses['chapters']}
        self.cache = {}

    def ordinary_words(self):
        """Every word the witnesses write in lowercase somewhere: an ordinary word of their languages, never a name."""
        if 'ordinary' not in self.cache:
            words = set()
            for key in self.order:
                for units in self.chapter(key)['units'].values():
                    for unit in units:
                        words.update(word for word in re.findall(r"[\w’'-]+", unit[2]) if word[:1].islower())
            self.cache['ordinary'] = words
        return self.cache['ordinary']

    def chapter(self, key):
        if key not in self.cache:
            self.cache[key] = read(os.path.join(self.home, 'witnesses', f'{key}.json'))
        return self.cache[key]

    @staticmethod
    def spans_text(chapter, language, spans):
        units = chapter['units'][language]
        return clean(' '.join(units[k][2][a:b] for k, a, b in spans))

    def places(self, forms):
        """The first place of a form in each part of the book: chapter, unit index, offsets of the sentence."""
        pattern = re.compile(r'(?<![\w])(?:' + '|'.join(sorted(map(re.escape, forms), key=len, reverse=True)) + r')(?![\w])')
        found = {}
        for key in self.order:
            part = key.split('.')[0]
            if part in found:
                continue
            chapter = self.chapter(key)
            for index, unit in enumerate(chapter['units']['de']):
                match = pattern.search(unit[2])
                if match:
                    found[part] = (key, index, match.start())
                    break
        return [found[part] for part in PARTS if part in found]

    def place_spans(self, forms):
        """The places of `places` with the unit's id and the matched word's offsets, for the reader to open."""
        out = []
        for key, index, at in self.places(forms):
            unit = self.chapter(key)['units']['de'][index]
            pattern = re.compile(r'(?:' + '|'.join(sorted(map(re.escape, forms), key=len, reverse=True)) + r')')
            match = pattern.match(unit[2], at)
            out.append({'chapter': key, 'unit': unit[0], 'start': at, 'end': match.end() if match else at})
        return out

    def sentence(self, key, index, at):
        """The aligned sentence pair around a place of the original: the original's sentence and each translation's."""
        chapter = self.chapter(key)
        out = {}
        for language in ('ru', 'en'):
            pairs = [pair for row in chapter.get('rows', []) for pair in row.get('sentences', {}).get(language, [])]
            for left, right in pairs:
                if any(k == index and a <= at < b for k, a, b in left):
                    out.setdefault('de', self.spans_text(chapter, 'de', left))
                    out[language] = self.spans_text(chapter, language, right)
                    break
        if 'de' not in out:
            out['de'] = clean(chapter['units']['de'][index][2])
        return out


def descent_text(origin, names, language):
    """The descent of a word in words: oldest form first, the parts each with the oldest form its own descent reaches."""
    if not origin:
        return None
    item = origin.get('origin', [{}])[0]
    steps = []
    for relation, code, term in item.get('chain', []):
        name = names.get(code, {}).get(language, code)
        steps.append(f'{name} {term}' + (f' ({relation})' if relation not in ('inherited',) else ''))
    text = ' → '.join(steps)
    if item.get('parts'):
        joined = ' + '.join(item['parts'])
        text = (text + '; ' if text else '') + ('сложено из ' if language == 'ru' else 'made of ') + joined
    return text or None


def read_clean(text):
    """A unit's text read whole (a line-end break joined, lines as spaces) with each character's offset in the unit."""
    out, positions, index = [], [], 0
    while index < len(text):
        if text[index] == '¬':
            index += 1
            while index < len(text) and text[index] in '\n ':
                index += 1
            continue
        char = text[index]
        if char.isspace():
            if out and out[-1] != ' ':
                out.append(' ')
                positions.append(index)
        else:
            out.append(char)
            positions.append(index)
        index += 1
    positions.append(len(text))
    return ''.join(out), positions


def key_places(home, term, book, limit=4):
    """The places by which the Tree reads the term: its concept's formulas by rank, or its figure's appearances, each
    as the aligned sentence that holds the formula (or a form of the word) with the Tree's own line on it."""
    concepts = {item['id']: item for item in read(os.path.join(home, 'core', 'concepts.json'))}
    figures = {item['id']: item for item in read(os.path.join(home, 'core', 'figures.json'))}
    forms = term['original']['forms']
    pattern = re.compile(r'(?<![\w])(?:' + '|'.join(sorted(map(re.escape, forms), key=len, reverse=True)) + r')(?![\w])')
    entity = concepts.get(term.get('concept')) or figures.get(term.get('concept')) or figures.get(term['id'])
    if not entity:
        return []
    cited = []
    for formula in sorted(entity.get('conception', {}).get('formulas', []), key=lambda item: item.get('rank', 99)):
        cited.append((formula['cite'], formula['cite'].get('q'), formula.get('why') or {}))
    for appearance in entity.get('appearances', []):
        cited.append((appearance['cite'], None, appearance.get('line') or {}))
    out, seen = [], set()
    for cite, quote, why in cited:
        key = cite.get('c')
        if key not in book.titles:
            continue
        units = book.chapter(key)['units']['de']
        ids = [unit[0] for unit in units]
        first, last = cite['u'][0], cite['u'][-1]
        if first not in ids:
            continue
        span = range(ids.index(first), (ids.index(last) if last in ids else ids.index(first)) + 1)
        found = []
        for index in span:
            text, positions = read_clean(units[index][2])
            if quote:
                said = clean(quote)
                at = text.find(said)
                if at >= 0:
                    found.append((index, positions[at], positions[at + len(said) - 1] + 1))
                    break
                continue
            found.extend((index, positions[match.start()], positions[match.end() - 1] + 1) for match in pattern.finditer(text))
        if quote and not found:
            # The formula's words broken otherwise than the quotation: the first form of the word in its units.
            for index in span:
                text, positions = read_clean(units[index][2])
                match = pattern.search(text)
                if match:
                    found.append((index, positions[match.start()], positions[match.end() - 1] + 1))
                    break
        for index, at, end in found:
            pair = book.sentence(key, index, at)
            if pair.get('de') in seen:
                continue
            seen.add(pair.get('de'))
            # `_unit`, `_start` and `_end` open the place in the reader; the packet keeps them out of what the writer reads.
            out.append({'chapter': key, 'title_de': clean(book.titles[key].get('de')), 'title_ru': clean(book.titles[key].get('ru')),
                        'title_en': clean(book.titles[key].get('en')), 'text': pair.get('de'), 'text_ru': pair.get('ru'),
                        'text_en': pair.get('en'), 'why_ru': why.get('ru'), 'why_en': why.get('en'),
                        '_unit': units[index][0], '_start': at, '_end': end})
            if len(out) == limit:
                return out
    return out


def frequency_text(attested, periods):
    """The word's frequency in the archive in a few figures: its first use, its highest quarter century, before and after 1875."""
    parts = []
    if attested.get('first'):
        parts.append(f"First use outside Nietzsche: {attested['first']}.")
    used = [period for period in periods if period[2]]
    if used:
        peak = max(used, key=lambda period: period[3])
        parts.append(f'Highest frequency: {peak[0]}–{peak[1]}, {peak[3]} per 10,000 nouns.')

    def mean(rows):
        return round(sum(row[3] for row in rows) / len(rows), 2) if rows else None
    before = mean([period for period in periods if 1600 <= period[0] < 1875])
    after = mean([period for period in periods if 1875 <= period[0] < 1925])
    if before is not None and after is not None:
        parts.append(f'Mean per 10,000 nouns: 1600–1874 {before}; 1875–1924 {after}.')
    return ' '.join(parts) or None


def packet(repo, home, term, book, names, renderings):
    lemma = term['original']['lemma']
    history = record(home, 'histories', 'de', lemma) or {}
    origin = record(home, 'origins', 'de', lemma) or {}
    sources = []

    def add(item):
        sources.append({key: value for key, value in item.items() if value not in (None, '', [], {})})

    for edition, told in sorted((history.get('told') or {}).items()):
        add({'id': f'W.{edition}', 'kind': 'wiktionary', 'about': lemma, 'edition': edition, 'entry': told['entry'],
             'url': EDITION_URL[edition].format(entry=told['entry'], section=SECTION['de']), 'text': told['text'],
             'basis': told.get('basis')})
    descent = {language: descent_text(origin, names, language) for language in ('ru', 'en')}
    if any(descent.values()):
        add({'id': 'E', 'kind': 'descent', 'about': lemma, 'text_ru': descent['ru'], 'text_en': descent['en']})
    attested = history.get('attested') or {}
    if attested.get('n'):
        manifest = read(os.path.join(home, 'histories.json'))['attested']
        step, start, nouns = manifest['periods']['step'], manifest['periods']['from'], manifest['nouns']
        periods = []
        for index, n in enumerate(attested['n']):
            year = attested['from'] + index * step
            size = nouns[(year - start) // step] if 0 <= (year - start) // step < len(nouns) else 0
            if size >= 30000:
                periods.append([year, year + step - 1, n, round(n * 10000 / size, 2)])
        add({'id': 'A', 'kind': 'archive-frequency', 'about': lemma, 'first': attested.get('first'),
             'first_period': attested.get('period'), 'text': frequency_text(attested, periods), 'periods': periods})
    for index, (year, author, title, text, span, basename, page, *rest) in enumerate(attested.get('early') or [], 1):
        add({'id': f'A{index}', 'kind': 'archive-use', 'about': lemma, 'year': year, 'author': author, 'title': title,
             'text': text, 'word': text[span[0]:span[1]], 'url': DTA_BOOK.format(basename=basename, page=page or 1)})
    if attested.get('nietzsche'):
        add({'id': 'N', 'kind': 'nietzsche-works', 'about': lemma,
             'text': '; '.join(f'{title}, {year}' for year, title in attested['nietzsche'])})
    counted = history.get('book') or {}
    if counted.get('n'):
        add({'id': 'B', 'kind': 'book-count', 'about': lemma,
             'text': f"{counted['n']} times in the book: " + ', '.join(f'part {part}: {n}' for part, n in counted['parts'].items())})
    for index, (key, unit, at) in enumerate(book.places(term['original']['forms']), 1):
        pair = book.sentence(key, unit, at)
        add({'id': f'B{index}', 'kind': 'book-place', 'chapter': key, 'title_de': clean(book.titles[key].get('de')),
             'title_ru': clean(book.titles[key].get('ru')), 'title_en': clean(book.titles[key].get('en')),
             'text': pair.get('de'), 'text_ru': pair.get('ru'), 'text_en': pair.get('en')})
    places_seen = {source.get('text') for source in sources if source.get('kind') == 'book-place'}
    for index, place in enumerate((item for item in key_places(home, term, book) if item['text'] not in places_seen), 1):
        add({'id': f'K{index}', 'kind': 'key-place', **{key: value for key, value in place.items() if not key.startswith('_')}})
    shown = renderings.get('terms', {}).get(term['id']) or {}
    counted_words = {language: (shown.get(language) or {}).get('words', [])[:4] for language in ('ru', 'en')}
    words = {language: ', '.join(f'{word} ({len(places)})' for word, places in counted_words[language]) for language in ('ru', 'en')}
    witness = {language: (term.get('renderings', {}).get(language, {}).get('witness') or {}) for language in ('ru', 'en')}
    tos = {language: (term.get('renderings', {}).get(language, {}).get('tos') or {}) for language in ('ru', 'en')}
    add({'id': 'T', 'kind': 'translations', 'about': lemma,
         'text': f"Antonovsky 1913, his words for it across the book (places): {words['ru'] or witness['ru'].get('term')}; "
                 f"at its first place: {witness['ru'].get('term')}. Common 1909: {words['en'] or witness['en'].get('term')}; "
                 f"at its first place: {witness['en'].get('term')}. The Tree's words: {tos['ru'].get('term')} (ru), {tos['en'].get('term')} (en).",
         'gloss_ru': term.get('gloss', {}).get('ru'), 'gloss_en': term.get('gloss', {}).get('en'), 'basis_en': tos['en'].get('basis')})
    # The Russian word whose etymology the packet gives: Antonovsky's commonest word for the term.
    russian = (counted_words['ru'][0][0] if counted_words['ru'] else None) or witness['ru'].get('term')
    if russian:
        told = (record(home, 'histories', 'ru', russian) or {}).get('told') or {}
        for edition, item in sorted(told.items()):
            add({'id': f'V.{edition}', 'kind': 'wiktionary', 'about': russian, 'edition': edition, 'entry': item['entry'],
                 'url': EDITION_URL[edition].format(entry=item['entry'], section=SECTION['ru']), 'text': item['text'],
                 'basis': item.get('basis')})
    body = {'schema': 'tos.bonsai.story-packet.v1', 'id': term['id'], 'lemma': lemma, 'language': 'de',
            'words': {'tree_ru': tos['ru'].get('term'), 'tree_en': tos['en'].get('term'), 'antonovsky': russian,
                      'common': witness['en'].get('term')},
            'sources': sources}
    body['digest'] = hashlib.sha256(json.dumps(body, ensure_ascii=False, sort_keys=True).encode()).hexdigest()
    return body


def packets(args):
    home = os.path.join(args.repo, 'bonsai', args.bonsai)
    terms = read(os.path.join(home, 'terms.json'))['terms']
    renderings = read(os.path.join(home, 'renderings.json'))
    names = language_names(args.repo)
    book = Book(home)
    wanted = set(args.terms.split(',')) if args.terms else None
    os.makedirs(os.path.join(args.out, 'packets'), exist_ok=True)
    written = []
    for term in terms:
        lemma = term['original']['lemma']
        if wanted and term['id'] not in wanted or ' ' in lemma or term['original'].get('lang') != 'de':
            continue
        if not record(home, 'histories', 'de', lemma):
            continue
        body = packet(args.repo, home, term, book, names, renderings)
        with open(os.path.join(args.out, 'packets', f"{term['id']}.json"), 'w', encoding='utf-8') as fh:
            json.dump(body, fh, ensure_ascii=False, indent=1)
            fh.write('\n')
        written.append(term['id'])
    print(json.dumps({'packets': len(written), 'terms': written}, ensure_ascii=False))


# ---------------------------------------------------------------- the check

QUOTES = re.compile(r'«([^«»]+)»|„([^„“]+)“|“([^“”]+)”|"([^"]+)"')
HEDGES = re.compile(r'(?i)\b(?:это не |не просто|не столько|скорее всего|по-видимому|можно сказать|как бы|стоит отметить|'
                    r'важно отметить|интересно, что|любопытно|not just|not merely|not only|it is worth|interestingly|'
                    r'arguably|perhaps|seems to|rather than)')
STAND_NAMES = {'Викисловарь', 'Викисловаря', 'Викисловаре', 'Викисловарём', 'Викисловари', 'Викисловарей', 'Wiktionary',
               'Немецкий', 'Немецкого', 'Немецком', 'Немецким', 'текстовый', 'архив', 'DTA', 'German', 'Text', 'Archive',
               'Ницше', 'Nietzsche', 'Заратустра', 'Заратустры', 'Заратустре', 'Заратустрой', 'Zarathustra', 'Zarathustra’s',
               'Антоновский', 'Антоновского', 'Антоновскому', 'Антоновским', 'Antonovsky', 'Коммон', 'Коммона', 'Коммону',
               'Common', 'Древо', 'Древа', 'Древе', 'Tree', 'Фасмер', 'Фасмера', 'Vasmer', 'Wiktionary’s', 'I', 'II', 'III', 'IV'}


def fold(text):
    text = unicodedata.normalize('NFC', text or '').replace('ſ', 's')
    return re.sub(r'\s+', ' ', text).strip()


def source_text(source):
    return ' '.join(str(value) for key, value in source.items() if key not in ('id', 'kind', 'url', 'periods'))


def words_of(sentence):
    return re.findall(r"[\w’'-]+", sentence)


def ground_numbers(ground):
    """Every whole number the pieces write, as written and with its separators dropped: a story's number matches one of them exactly."""
    numbers = set()
    for found in re.findall(r'\d+(?:[.,]\d+)*', ground):
        numbers.update({found, found.replace(',', '.'), re.sub(r'[.,]', '', found)})
        numbers.update(re.findall(r'\d+', found))
    # A thousands separator written as a space ("12 345") joins its digits.
    numbers.update(re.sub(r'\s', '', found) for found in re.findall(r'\d{1,3}(?:\s\d{3})+(?!\d)', ground))
    return numbers


def batch_ordinary(book, directory):
    """The ordinary words a sentence may open with: the witnesses' lowercase words, those of the packets and those inside the stories."""
    words = set(book.ordinary_words())
    for path in glob.glob(os.path.join(directory, 'packets', '*.json')):
        for source in read(path)['sources']:
            for value in source.values():
                if isinstance(value, str):
                    words.update(word for word in words_of(value) if word[:1].islower())
    for path in glob.glob(os.path.join(directory, 'stories', '*.json')):
        story = read(path)
        for language in ('ru', 'en'):
            for item in story.get(language) or []:
                words.update(word for word in words_of(item.get('text', ''))[1:] if word[:1].islower())
    return words


def check_story(packet_body, story, names, ordinary=frozenset()):
    problems, warnings = [], []
    by_id = {source['id']: source for source in packet_body['sources']}
    allowed = set(STAND_NAMES)
    for code in names.values():
        for name in code.values():
            allowed.update(name.split())
    for value in packet_body.get('words', {}).values():
        if value:
            allowed.update(value.split())
    declared = {}
    for item in story.get('names') or []:
        declared.setdefault(item.get('text'), []).append(item.get('source'))
    if story.get('packet') != packet_body['digest']:
        problems.append('the story names another packet')
    for language in ('ru', 'en'):
        sentences = story.get(language) or []
        if not sentences:
            problems.append(f'{language}: no story')
            continue
        total = sum(len(words_of(item.get('text', ''))) for item in sentences)
        if total > LIMIT_WORDS:
            problems.append(f'{language}: {total} words, more than {LIMIT_WORDS}')
        for number, item in enumerate(sentences, 1):
            text, cited = item.get('text', ''), item.get('sources') or []
            where = f'{language} sentence {number}'
            unknown = [source for source in cited if source not in by_id]
            if not cited or unknown:
                problems.append(f'{where}: cites {unknown or "nothing"}')
                continue
            ground = fold(' '.join(source_text(by_id[source]) for source in cited))
            numbers = ground_numbers(ground)
            for found in re.findall(r'\d+(?:[.,]\d+)?', text):
                if not {found, found.replace(',', '.'), re.sub(r'[.,]', '', found)} & numbers:
                    problems.append(f'{where}: the number {found} stands in none of {cited}')
            quoted = []
            for match in QUOTES.finditer(text):
                inner = fold(next(group for group in match.groups() if group)).strip(' .,;:!?…')
                quoted.append(match.span())
                # A title written in the story's language stands quoted as a declared name with its source form.
                titled = any(source and fold(source) in ground for source in declared.get(inner, []))
                if inner and inner not in ground and not titled:
                    problems.append(f'{where}: the quotation «{inner}» stands in none of {cited}')
            outside = text
            for start, end in reversed(quoted):
                outside = outside[:start] + ' ' * (end - start) + outside[end:]
            tokens = list(re.finditer(r"[\w’'-]+", outside))
            for index, token in enumerate(tokens):
                word = token.group(0).strip("’'-")
                if not word or not word[0].isupper():
                    continue
                if word in allowed or word in ground or any(source and fold(source) in ground for source in declared.get(word, [])):
                    continue
                # A sentence opens with a capital: its first word stands as an ordinary word when the witnesses write it in lowercase.
                if index == 0 and word.lower() in ordinary:
                    continue
                problems.append(f'{where}: the name {word} stands in none of {cited} and the story does not declare it')
            if HEDGES.search(text):
                problems.append(f'{where}: hedging or a negation of the AI kind: «{HEDGES.search(text).group(0)}»')
    return problems, warnings


def check(args):
    names = language_names(args.repo)
    ordinary = batch_ordinary(Book(os.path.join(args.repo, 'bonsai', args.bonsai)), args.dir)
    report, counts = {}, Counter()
    for path in sorted(glob.glob(os.path.join(args.dir, 'packets', '*.json'))):
        term = os.path.splitext(os.path.basename(path))[0]
        story_path = os.path.join(args.dir, 'stories', f'{term}.json')
        if not os.path.exists(story_path):
            counts['missing'] += 1
            continue
        problems, warnings = check_story(read(path), read(story_path), names, ordinary)
        report[term] = {'ok': not problems, 'problems': problems, 'warnings': warnings}
        counts['passed' if not problems else 'failed'] += 1
    with open(os.path.join(args.dir, 'check.json'), 'w', encoding='utf-8') as fh:
        json.dump({'counts': dict(counts), 'stories': report}, fh, ensure_ascii=False, indent=1)
        fh.write('\n')
    for term, item in report.items():
        if item['problems']:
            print(f'{term}:')
            for problem in item['problems']:
                print(f'  - {problem}')
    print(json.dumps(dict(counts)))
    return 0 if not counts['failed'] and not counts['missing'] else 1


def shown_piece(source, extra):
    """A cited piece as the stand shows it; the archive's frequency and uses the origin sheet shows by itself."""
    kind = source['kind']
    if kind in ('book-place', 'key-place'):
        piece = {'kind': kind, **extra.get(source['id'], {}),
                 'title': {'de': source.get('title_de'), 'ru': source.get('title_ru'), 'en': source.get('title_en')},
                 'text': {'de': source.get('text'), 'ru': source.get('text_ru'), 'en': source.get('text_en')}}
        if source.get('why_ru') or source.get('why_en'):
            piece['why'] = {'ru': source.get('why_ru'), 'en': source.get('why_en')}
        return piece
    if kind == 'translations':
        return {'kind': kind, **extra.get('T', {})}
    if kind == 'wiktionary':
        return {key: source.get(key) for key in ('kind', 'about', 'edition', 'entry', 'url', 'text', 'basis') if source.get(key)}
    if kind == 'descent':
        return {'kind': kind, 'text': {'ru': source.get('text_ru'), 'en': source.get('text_en')}}
    return {'kind': kind}


def display_extra(home, term, book, renderings):
    """What the stand needs to show a piece beyond the packet: a place's unit and offsets, the translators' words by count."""
    extra = {f'B{index}': place for index, place in enumerate(book.place_spans(term['original']['forms']), 1)}
    places = book.place_spans(term['original']['forms'])
    seen = {book.sentence(item['chapter'], [unit[0] for unit in book.chapter(item['chapter'])['units']['de']].index(item['unit']), item['start']).get('de') for item in places}
    for index, place in enumerate((item for item in key_places(home, term, book) if item['text'] not in seen), 1):
        extra[f'K{index}'] = {'chapter': place['chapter'], 'unit': place['_unit'], 'start': place['_start'], 'end': place['_end']}
    shown = renderings.get('terms', {}).get(term['id']) or {}
    extra['T'] = {
        'words': {language: [[word, len(places)] for word, places in (shown.get(language) or {}).get('words', [])[:4]] for language in ('ru', 'en')},
        'first': {language: (term.get('renderings', {}).get(language, {}).get('witness') or {}).get('term') for language in ('ru', 'en')},
        'tree': {language: (term.get('renderings', {}).get(language, {}).get('tos') or {}).get('term') for language in ('ru', 'en')}}
    return extra


def bundle(args):
    home = os.path.join(args.repo, 'bonsai', args.bonsai)
    names = language_names(args.repo)
    terms = {term['id']: term for term in read(os.path.join(home, 'terms.json'))['terms']}
    renderings = read(os.path.join(home, 'renderings.json'))
    book = Book(home)
    ordinary = batch_ordinary(book, args.dir)
    stories, counts, refused = {}, Counter(), {}
    told = {os.path.splitext(os.path.basename(path))[0] for path in glob.glob(os.path.join(args.dir, 'stories', '*.json'))}
    for path in sorted(glob.glob(os.path.join(args.dir, 'packets', '*.json'))):
        term_id = os.path.splitext(os.path.basename(path))[0]
        if term_id not in told:
            counts['missing'] += 1
            refused[term_id] = 'no story'
    for path in sorted(glob.glob(os.path.join(args.dir, 'stories', '*.json'))):
        term_id = os.path.splitext(os.path.basename(path))[0]
        packet_path = os.path.join(args.dir, 'packets', f'{term_id}.json')
        if not os.path.exists(packet_path) or term_id not in terms:
            counts['without_packet'] += 1
            refused[term_id] = 'no packet or no term'
            continue
        packet_body, story = read(packet_path), read(path)
        problems, _ = check_story(packet_body, story, names, ordinary)
        if problems:
            counts['failed'] += 1
            refused[term_id] = 'fails the check'
            continue
        # The pieces as this checkout gives them: a place of the book with its unit, the translators' words by count.
        current = packet(args.repo, home, terms[term_id], book, names, renderings)
        if current['digest'] != packet_body['digest']:
            counts['stale'] += 1
            refused[term_id] = 'stands on an earlier packet'
            continue
        by_id = {source['id']: source for source in current['sources']}
        extra = display_extra(home, terms[term_id], book, renderings)
        cited = []
        for language in ('ru', 'en'):
            for sentence in story[language]:
                cited.extend(source for source in sentence['sources'] if source not in cited)
        stories[packet_body['lemma']] = {
            'id': term_id, 'packet': packet_body['digest'],
            'ru': [{'text': item['text'], 'sources': item['sources']} for item in story['ru']],
            'en': [{'text': item['text'], 'sources': item['sources']} for item in story['en']],
            'pieces': {source: shown_piece(by_id[source], extra) for source in cited}}
        counts['stories'] += 1
    if refused:
        # The layer holds every term's story or none: a partial layer would publish gaps as the stand's whole.
        for term_id, reason in sorted(refused.items()):
            print(f'{term_id}: {reason}')
        print(json.dumps(dict(sorted(counts.items()))))
        return 1
    witnesses = read(os.path.join(home, 'witnesses.json'))
    manifest = {
        'schema': 'tos.bonsai.stories.v1', 'work': witnesses.get('work'), 'original': 'de',
        'posture': {
            'ru': 'Короткая история слова каждого термина, рассказанная только по собранным источникам: каждое предложение несёт куски, на которых стоит; проверка сверила числа, цитаты и имена с этими кусками, вторая модель вычитала текст. Предложение стенда.',
            'en': "The short story of each term's word, told from its gathered sources only: every sentence carries the pieces it stands on; the check matched its numbers, quotations and names against them, and a second model read the text. The stand's proposal."},
        'writer': {'model': args.writer, 'brief': 'bonsai/review/philology/BRIEF-word-stories.md'},
        'check': 'bonsai/review/philology/stories.py check', 'reviewer': args.reviewer,
        'licence': 'CC BY-SA 4.0', 'licence_basis': "Told from Wiktionary's etymologies and the Deutsches Textarchiv's texts (CC BY-SA 4.0), with the witnesses' sentences under their own licences in witnesses.json.",
        'counts': dict(sorted(counts.items())), 'stories': dict(sorted(stories.items()))}
    with open(os.path.join(home, 'stories.json'), 'w', encoding='utf-8') as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=1)
        fh.write('\n')
    print(json.dumps(manifest['counts']))
    return 0


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest='command', required=True)
    one = sub.add_parser('packets')
    one.add_argument('repo')
    one.add_argument('bonsai')
    one.add_argument('--out', required=True)
    one.add_argument('--terms')
    two = sub.add_parser('check')
    two.add_argument('dir')
    two.add_argument('--repo', default=os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..')))
    two.add_argument('--bonsai', default='zarathustra')
    three = sub.add_parser('bundle')
    three.add_argument('dir')
    three.add_argument('repo')
    three.add_argument('bonsai')
    three.add_argument('--writer', default='gpt-6.1-terra (high)')
    three.add_argument('--reviewer', default='claude-opus-5-5')
    args = ap.parse_args()
    return {'packets': packets, 'check': check, 'bundle': bundle}[args.command](args)


if __name__ == '__main__':
    sys.exit(main())
