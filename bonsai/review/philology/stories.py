#!/usr/bin/env python3
"""The short story of a word, told from its gathered sources only, and the check it passes.

    python3 stories.py packets <repo> <bonsai> --out <dir> [--terms id,id,…]
    python3 stories.py check <dir>

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
- every number of a sentence stands in the pieces it names;
- every quotation stands word for word in the pieces it names;
- every name a sentence writes stands in those pieces, among the names the story
  declares with their source form, or among the names the stand reads by itself
  (languages, Wiktionary, the archive, Nietzsche, Zarathustra, the book's
  chapters, the translators);
- a story keeps its length and its language to affirmation.

It writes `<dir>/check.json` and prints a summary. The story is the writer's
proposal; the check and a reviewing model stand between it and the stand.
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
             'first_period': attested.get('period'),
             'text': 'Uses outside Nietzsche per quarter century [from, to, uses, per 10,000 nouns]: '
                     + '; '.join(f'{a}–{b}: {n} ({rate})' for a, b, n, rate in periods),
             'periods': periods})
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
    shown = renderings.get('terms', {}).get(term['id']) or {}
    words = {language: ', '.join(f'{word} ({len(places)})' for word, places in (shown.get(language) or {}).get('words', [])[:4])
             for language in ('ru', 'en')}
    witness = {language: (term.get('renderings', {}).get(language, {}).get('witness') or {}) for language in ('ru', 'en')}
    tos = {language: (term.get('renderings', {}).get(language, {}).get('tos') or {}) for language in ('ru', 'en')}
    add({'id': 'T', 'kind': 'translations', 'about': lemma,
         'text': f"Antonovsky 1913: {witness['ru'].get('term')}; Common 1909: {witness['en'].get('term')}; "
                 f"the Tree's words: {tos['ru'].get('term')} (ru), {tos['en'].get('term')} (en)",
         'words_ru': words['ru'], 'words_en': words['en'], 'gloss_ru': term.get('gloss', {}).get('ru'),
         'gloss_en': term.get('gloss', {}).get('en'), 'basis_en': tos['en'].get('basis')})
    russian = witness['ru'].get('term')
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


def check_story(packet_body, story, names):
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
            digits = re.sub(r'(?<=\d)[\s.,](?=\d)', '', ground)
            for found in re.findall(r'\d+(?:[.,]\d+)?', text):
                plain = re.sub(r'[.,]', '', found)
                if plain not in digits and found not in ground and found.replace(',', '.') not in ground:
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
                before = outside[:token.start()].rstrip()
                if index == 0 or before.endswith(('.', '!', '?', ':', '—', '(')) and language == 'en' and index == 0:
                    continue
                if index == 0:
                    continue
                if word in allowed or word in ground or any(source and fold(source) in ground for source in declared.get(word, [])):
                    continue
                problems.append(f'{where}: the name {word} stands in none of {cited} and the story does not declare it')
            if HEDGES.search(text):
                problems.append(f'{where}: hedging or a negation of the AI kind: «{HEDGES.search(text).group(0)}»')
    return problems, warnings


def check(args):
    names = language_names(args.repo)
    report, counts = {}, Counter()
    for path in sorted(glob.glob(os.path.join(args.dir, 'packets', '*.json'))):
        term = os.path.splitext(os.path.basename(path))[0]
        story_path = os.path.join(args.dir, 'stories', f'{term}.json')
        if not os.path.exists(story_path):
            counts['missing'] += 1
            continue
        problems, warnings = check_story(read(path), read(story_path), names)
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
    return 0 if not counts['failed'] else 1


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
    args = ap.parse_args()
    return {'packets': packets, 'check': check}[args.command](args)


if __name__ == '__main__':
    sys.exit(main())
