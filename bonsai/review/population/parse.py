#!/usr/bin/env python3
"""Parse the persons register rows into one shape: names, kind, identifiers, life dates, gaps.

    python3 -I bonsai/review/population/parse.py [--check]

Reads registry/<batch>/agents.jsonl (written by intake.py) and writes layer/agents/<batch>.jsonl:
one record per register row, in the row order of its sheet. A record keeps what the identity
resolution and the planting need and says, under `gaps`, what the row leaves open or what this
parser could not read. The parser decides nothing about identity: a record is one row.

Every rule here reads only the words of the row. A kind other than `person` needs an explicit
phrase of the register (`kind_basis` quotes it); a weaker phrase becomes a `signals` entry for a
reader. A life date is read only from the register's birth, death, life and activity lines, and
only in the forms below; any other form stays a gap with its line kept verbatim.

    year         1848 · около 429 до н. э. · 1530?           (year, approximate, uncertain)
    date         08.11.1848 · 1848-11-08 · 1848-11
    range        497–425 гг. до н. э. · ок. 480–540
    century      XI в. · V в. до н. э. · XIII–XIV вв.

Years are astronomical (1 BCE = 0). A year below 1000 in an antiquity batch (A..) needs an era in
its line, or it stays a gap. Only the standard library is used.
"""

import argparse
import json
import os
import re
import sys
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------- scripts and forms

SCRIPT_PREFIXES = (
    ('LATIN', 'Latn'), ('CYRILLIC', 'Cyrl'), ('GREEK', 'Grek'), ('ARABIC', 'Arab'), ('HEBREW', 'Hebr'),
    ('CJK', 'Hani'), ('HIRAGANA', 'Jpan'), ('KATAKANA', 'Jpan'), ('HANGUL', 'Kore'),
    ('DEVANAGARI', 'Deva'), ('BENGALI', 'Beng'), ('TAMIL', 'Taml'), ('TELUGU', 'Telu'), ('KANNADA', 'Knda'),
    ('MALAYALAM', 'Mlym'), ('GUJARATI', 'Gujr'), ('GURMUKHI', 'Guru'), ('ORIYA', 'Orya'), ('SINHALA', 'Sinh'),
    ('TIBETAN', 'Tibt'), ('MONGOLIAN', 'Mong'), ('ETHIOPIC', 'Ethi'), ('ARMENIAN', 'Armn'), ('GEORGIAN', 'Geor'),
    ('SYRIAC', 'Syrc'), ('COPTIC', 'Copt'), ('THAI', 'Thai'), ('LAO', 'Laoo'), ('KHMER', 'Khmr'), ('MYANMAR', 'Mymr'),
    ('CUNEIFORM', 'Xsux'), ('EGYPTIAN HIEROGLYPH', 'Egyp'), ('TANGUT', 'Tang'), ('YI ', 'Yiii'), ('OLD ', 'Old'),
)
_script_cache = {}


def char_script(ch):
    if ch in _script_cache:
        return _script_cache[ch]
    name = unicodedata.name(ch, '')
    script = None
    for prefix, code in SCRIPT_PREFIXES:
        if name.startswith(prefix):
            script = code
            break
    if script is None and ch.isalpha():
        script = 'Other'
    _script_cache[ch] = script
    return script


def script_of(text):
    counts = {}
    for ch in unicodedata.normalize('NFC', text):
        script = char_script(ch)
        if script:
            counts[script] = counts.get(script, 0) + 1
    if not counts:
        return None
    return max(sorted(counts), key=lambda key: counts[key])


PLACEHOLDER = re.compile(r'(?i)^(?:not_checked|not checked|unknown|not_applicable|not applicable|n/?a|нет|—|-|–|\?)\b')
RU_WRAPPERS = (
    re.compile(r'^Участник авторской/редакторской строки:\s*(?P<name>.+?)(?:\s*\([^()]*\))?\s*$'),
    re.compile(r'^Персона:\s*(?P<name>.+?)\s*$'),
    re.compile(r'^Автор,\s*указанный в \S+ как\s*«(?P<name>[^»]+)».*$'),
)
VERSE_SUFFIX = re.compile(r'\s+[—–-]\s+(?P<ref>TH[AI](?:G\d+\.\d+|-AP\d+))\s*$')


def clean(text):
    return re.sub(r'\s+', ' ', unicodedata.normalize('NFC', text)).strip()


def ru_name(label):
    """The Russian label without the register's wrappers; the verse a voice is bound to, if any."""
    text, ref = clean(label), None
    match = VERSE_SUFFIX.search(text)
    if match:
        ref, text = match.group('ref'), text[:match.start()].strip()
    for pattern in RU_WRAPPERS:
        match = pattern.match(text)
        if match:
            text = match.group('name').strip()
            break
    return text, ref


def lang_tag(field):
    field = clean(field)
    field = re.sub(r'^(?:язык|language)\s*[:=]\s*', '', field, flags=re.I)
    match = re.match(r'^([a-z]{2,3}(?:-[A-Za-z]{2,8})*)\b', field)
    return match.group(1) if match else None


def variant_forms(text):
    forms = []
    for line in text.split('\n'):
        line = line.strip()
        if not line:
            continue
        sep = '|' if line.count('|') > line.count(';') else ';'
        fields = [clean(part) for part in line.split(sep)]
        head = fields[0]
        if not head or PLACEHOLDER.match(head):
            continue
        lang = lang_tag(fields[1]) if len(fields) > 1 else None
        for part in re.split(r'\s+/\s+', head):
            part = part.strip(' ,;')
            if not part or len(part) > 100 or len(part.split()) > 10 or PLACEHOLDER.match(part):
                continue
            forms.append({'text': part, 'lang': lang})
    return forms


# ---------------------------------------------------------------- kinds

KIND_RULES = (
    ('attributed-voice', 'disambiguation_note', re.compile(r'(?i)традиционная человеческая атрибутивная идентичность')),
    ('pseudonymous', 'preferred_label', re.compile(r'(?i)(?:^|[\s(])(?:pseudo-|псевдо-)|\(pseudo-?\)')),
    ('pseudonymous', 'label_ru', re.compile(r'(?i)(?:^|[\s(])псевдо-')),
    ('pseudonymous', 'status', re.compile(r'(?i)псевдоним')),
    ('unnamed-hand', 'preferred_label', re.compile(r'(?i)^(?:anonymous|anonymus|аноним|неизвестный (?:автор|писец|переводчик)|unknown (?:author|scribe|translator)|безымянн)')),
    ('unnamed-hand', 'status', re.compile(r'(?i)условная писцовая идентичность|личное имя unknown')),
    ('collective', 'preferred_label', re.compile(r'(?i)^(?:коллектив|редакци|редакционн|committee|editorial board|institute|society|association|commission|council|комитет|комиссия|институт|общество|академия)\b')),
    ('conventional', 'status', re.compile(r'(?i)условная авторская идентичность|традиционная авторская(?:/учительская)? идентичность|ритуальная авторская идентичность|фигура ранней традиции')),
    ('conventional', 'disambiguation_note', re.compile(r'(?i)псевдоэпиграфическ\w* (?:авторск|голос)|традиционная авторская идентичность|легендарн\w* (?:автор|фигур|мудрец)|мифическ\w* (?:автор|фигур)')),
)
SIGNAL_RULES = (
    ('conventional?', re.compile(r'(?i)традиционн\w* атрибуц|легендарн|мифическ|условн\w* (?:фигур|идентичност)|литературная фигура')),
    ('collective?', re.compile(r'(?i)коллективн\w* (?:автор|идентичност)|группа авторов|авторский коллектив|corporate author')),
    ('unnamed?', re.compile(r'(?i)безымянн\w* (?:автор|писец|переводчик)|анонимн\w* (?:автор|составител)')),
    ('combined?', re.compile(r'(?i)объединённ\w* (?:строк|байлайн)|несколько лиц в одной|combined byline')),
)
STATUS_PHRASE = re.compile(r'Статус (?:фигуры|лица|идентичности)\s*:\s*([^.;\n]+)')
KIND_TREE_TYPES = {'person': 'tos.entity.agent', 'pseudonymous': 'tos.entity.agent', 'conventional': 'tos.entity.agent',
                   'attributed-voice': 'tos.entity.agent', 'unnamed-hand': 'tos.entity.agent', 'collective': 'tos.entity.organization'}


def kind_of(row):
    status = ' '.join(m.group(1) for field in ('notes', 'disambiguation_note', 'responsibility_summary')
                      for m in STATUS_PHRASE.finditer(row.get(field, '')))
    sources = dict(row, status=status)
    for kind, field, pattern in KIND_RULES:
        match = pattern.search(sources.get(field, ''))
        if match:
            return kind, {'field': field, 'words': match.group(0).strip()}
    if row.get('record_type', '').strip() not in ('agent', 'person'):
        return 'person', {'field': 'record_type', 'words': row.get('record_type', '')}
    return 'person', None


def signals_of(row, kind):
    text = ' '.join(row.get(field, '') for field in ('preferred_label', 'notes', 'disambiguation_note'))
    found = []
    for name, pattern in SIGNAL_RULES:
        if pattern.search(text) and not (kind != 'person' and name.rstrip('?') in kind):
            found.append(name)
    label = clean(row.get('preferred_label', ''))
    sides = [side.split() for side in label.split(', ')] if ', ' in label else []
    if kind == 'person' and (re.search(r'\s(?:and|&|и)\s', label) or label.count(',') >= 2
                             or (len(sides) == 2 and all(len(side) >= 2 for side in sides))):
        found.append('combined?')
    return sorted(set(found))


# ---------------------------------------------------------------- identifiers

def mod11_2(digits):
    total = 0
    for ch in digits[:-1]:
        total = (total + int(ch)) * 2
    check = (12 - total % 11) % 11
    return digits[-1] == ('X' if check == 10 else str(check))


ID_PATTERNS = (
    ('orcid', re.compile(r'\b(\d{4}-\d{4}-\d{4}-\d{3}[\dX])\b')),
    ('wikidata', re.compile(r'wikidata\.org/(?:wiki|entity)/(Q\d+)\b')),
    ('viaf', re.compile(r'viaf\.org/(?:en/)?viaf/(\d+)')),
    ('viaf', re.compile(r'\bVIAF\s*[;:]?\s*(\d{4,})\b')),
    ('lccn', re.compile(r'id\.loc\.gov/authorities/names/(n[a-z]{0,2}\d{6,10})')),
    ('lccn', re.compile(r'\bLCCN\s*[;:]?\s*(n[a-z]{0,2}\s?\d{6,10})\b')),
    ('gnd', re.compile(r'd-nb\.info/gnd/([0-9X-]{6,12})')),
    ('gnd', re.compile(r'deutsche-biographie\.de/(?:pnd|gnd)([0-9X-]{6,12})\.html')),
    ('gnd', re.compile(r'\bGND\s*[;:]?\s*([0-9]{6,10}[0-9X]|[0-9]{1,9}-[0-9X])\b')),
    ('isni', re.compile(r'isni\.org/isni/(\d{15}[\dX])')),
    ('isni', re.compile(r'\bISNI\s*[;:]?\s*(\d{4}\s?\d{4}\s?\d{4}\s?\d{3}[\dX])\b')),
    ('bnf', re.compile(r'catalogue\.bnf\.fr/ark:/12148/(cb\d{8}[0-9bcdfghjkmnpqrstvwxz])')),
    ('dila', re.compile(r'authority\.dila\.edu\.tw/person/\?fromInner=(A\d{6})')),
    ('dila', re.compile(r'\bDILA[^;\n]{0,40}?[;:]\s*(A\d{6})\b')),
    ('cdli-author', re.compile(r'cdli\.earth/authors/(\d+)')),
    ('bdrc', re.compile(r'(?:bdrc\.io|tbrc\.org)/[^\s;]*?\b(P\d{2,6})\b')),
)


def identifiers(text):
    found = {}
    for scheme, pattern in ID_PATTERNS:
        for match in pattern.finditer(text):
            value = re.sub(r'\s', '', match.group(1))
            found[(scheme, value)] = True
    out = []
    for scheme, value in sorted(found):
        item = {'scheme': scheme, 'value': value}
        if scheme == 'orcid':
            item['valid'] = mod11_2(value.replace('-', ''))
        elif scheme == 'isni':
            item['valid'] = mod11_2(value)
        out.append(item)
    return out


# ---------------------------------------------------------------- life dates

BIRTH, DEATH, LIFE, ACTIVITY, STATUS = 'birth', 'death', 'life', 'activity', 'status'


def line_kind(word):
    word = clean(word).lower().rstrip('.').strip()
    if not word:
        return None
    if word.startswith(('статус жизни', 'жизненный статус', 'актуальный жизненный статус', 'current snapshot', 'source snapshot')):
        return STATUS
    if word in ('рождение', 'birth', 'родился', 'родилась'):
        return BIRTH
    if word.startswith('смерть') or word in ('death', 'умер', 'умерла'):
        return DEATH
    if word in ('жизнь', 'годы жизни', 'жизненная хронология', 'период жизни', 'рождение и смерть', 'life'):
        return LIFE
    if re.search(r'литературная локализация|традиционный горизонт|хронологический ориентир|предполагаем|посмертн', word):
        return None
    if re.search(r'деятельност|floruit|флор|акме|\bfl\.?$', word):
        return ACTIVITY
    return None


def life_entries(text):
    """(kind, kind words, value, [other fields], line) for every life line a parser can place."""
    entries = []
    for raw in text.split('\n'):
        line = raw.strip()
        if not line:
            continue
        sep = '|' if line.count('|') > line.count(';') else ';'
        fields = [part.strip() for part in line.split(sep)]
        current = None
        for field in fields:
            match = re.match(r'^([^:]{2,70}?)\s*:\s*(.+)$', field)
            if match and line_kind(match.group(1)) is not None:
                current = [line_kind(match.group(1)), match.group(1), match.group(2).strip(), [], line]
                entries.append(current)
            elif current is None:
                kind = line_kind(field)
                if kind is None and len(fields) == 1:
                    kind = LIFE if re.match(r'(?i)^(?:unknown|not_checked|not checked|не установлен)', field) else None
                    current = [kind, '', field, [], line]
                else:
                    current = [kind, field, None, [], line]
                entries.append(current)
            elif current[2] is None:
                current[2] = field
            else:
                current[3].append(field)
    return [tuple(entry) for entry in entries]


ROMAN = re.compile(r'^(?=[IVXL])(L?X{0,3})(IX|IV|V?I{0,3})$')
ERA_BCE = r'(?:до\s*н\.?\s*э\.?|до\s+нашей\s+эры|до\s+Р\.?\s*Х\.?|BCE|B\.C\.E?\.?|BC)'
ERA_CE = r'(?:н\.?\s*э\.?|нашей\s+эры|CE|C\.E\.|AD|A\.D\.)'
APPROX = r'(?:около|ок\.|приблизительно|примерно|ca\.|c\.|circa)'
YEAR = re.compile(rf'^(?P<approx>{APPROX}\s*)?(?P<y>\d{{1,4}})(?P<q>\?)?(?:\s*г(?:г)?\.)?(?:\s*(?P<era>{ERA_BCE}|{ERA_CE}))?$', re.I)
RANGE = re.compile(rf'^(?P<approx>{APPROX}\s*)?(?P<a>\d{{1,4}})(?P<qa>\?)?(?:\s*(?P<ea>{ERA_BCE}|{ERA_CE}))?\s*[–—-]\s*(?:{APPROX}\s*)?(?P<b>\d{{1,4}})(?P<qb>\?)?(?:\s*г(?:г)?\.)?(?:\s*(?P<era>{ERA_BCE}|{ERA_CE}))?$', re.I)
DATE_DMY = re.compile(r'^(?P<d>\d{1,2})\.(?P<m>\d{1,2})\.(?P<y>\d{3,4})(?:\s*г\.)?$')
DATE_ISO = re.compile(r'^(?P<y>\d{3,4})-(?P<m>\d{2})(?:-(?P<d>\d{2}))?$')
HIJRI_MARK = r'(?:AH|A\.H\.|х\.|г\.\s*х\.|хиджры)'
HIJRI_PAIR = re.compile(rf'^(?P<x>\d{{1,4}})\s*(?P<xa>{HIJRI_MARK})?\s*/\s*(?P<y>\d{{1,4}})\s*(?P<ya>{HIJRI_MARK})?(?:\s*г\.)?$', re.I)
CHECK_DATE = re.compile(r'(?:статус[^;\n]*?\bна|проверено|сверк\w*|на дату\w*|snapshot)\s*(?:на\s*)?(\d{4}-\d{2}-\d{2})', re.I)
CENTURY = re.compile(rf'^(?P<approx>{APPROX}\s*)?(?P<c>[IVXL]+)(?:\s*[–—-]\s*(?P<c2>[IVXL]+))?\s*(?:в\.|вв\.|век|века|веков)(?:\s*(?P<era>{ERA_BCE}|{ERA_CE}))?$', re.I)
OPEN_UNKNOWN = re.compile(r'(?i)^(?:unknown|неизвестн|не установлен|не известн|дата не установлена|годы\s+\S+\s+неизвестн|not established)')
OPEN_UNCHECKED = re.compile(r'(?i)^(?:not_checked|not checked|не проверен|не сверен)')
OPEN_NA = re.compile(r'(?i)^(?:not_applicable|not applicable|неприменимо)')


def roman(text):
    text = text.upper()
    if not ROMAN.match(text):
        return None
    values = {'I': 1, 'V': 5, 'X': 10, 'L': 50}
    total = 0
    for i, ch in enumerate(text):
        value = values[ch]
        total += -value if i + 1 < len(text) and values[text[i + 1]] > value else value
    return total


def lexeme(year, month=None, day=None):
    text = ('-' if year < 0 else '') + f'{abs(year):04d}'
    if month:
        text += f'-{month:02d}'
        if day:
            text += f'-{day:02d}'
    return text


def leap(year):
    return year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)


def day_valid(year, month, day):
    days = [31, 29 if leap(year) else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    return 1 <= month <= 12 and 1 <= day <= days[month - 1]


def era_of(match_era, others):
    """+1 for CE, -1 for BCE, None where neither the value nor the line names one."""
    if match_era:
        return -1 if re.match(ERA_BCE, match_era, re.I) else 1
    for field in others:
        if re.search(rf'(?:^|\s|;){ERA_BCE}(?:$|\s|[;,.])', field, re.I):
            return -1
        if re.search(rf'(?:^|\s|;){ERA_CE}(?:$|\s|[;,.])', field, re.I):
            return 1
    return None


def astronomical(year, era):
    return 1 - year if era == -1 else year


def certainty_of(value, others, approx=False, query=False):
    text = ' '.join([value] + list(others)).lower()
    if query or '?' in value or re.search(r'предположительн|disputed|спорн|гипотетич|uncertain|неуверенн', text):
        return 'uncertain'
    if approx or re.search(r'приблизительн|около|\bок\.|circa|\bca\.', text):
        return 'approximate'
    return 'exact'


def hijri_to_ce(year):
    return year * 0.970229 + 621.5643


def point(year, era, month, day, certainty):
    """A dated point; `era` is the era the line names (None where CE is taken by default)."""
    astro = astronomical(year, era or 1)
    if month and day and not day_valid(astro, month, day):
        day = None
    result = {'kind': 'point', 'value': lexeme(astro, month, day), 'year': astro, 'certainty': certainty}
    if era is not None:
        result['era'] = era
    return result


def read_value(value, others, antiquity):
    """A parsed dating: {'kind': 'point'|'span'|'unknown'|'unchecked'|'na'|'free'|'era', ...}."""
    value = clean(value or '').rstrip(';,')
    value = re.sub(r'^(?:значение|value)\s*[=:]\s*', '', value, flags=re.I)
    value_core = re.sub(r'\.$', '', value) if not re.search(rf'{ERA_BCE}$|{ERA_CE}$|в\.$|вв\.$|г\.$', value, re.I) else value
    if not value:
        return {'kind': 'free'}
    if OPEN_UNCHECKED.match(value):
        return {'kind': 'unchecked'}
    if OPEN_NA.match(value):
        return {'kind': 'na'}
    if OPEN_UNKNOWN.match(value):
        return {'kind': 'unknown'}
    match = DATE_DMY.match(value_core) or DATE_ISO.match(value_core)
    if match:
        year, month = int(match.group('y')), int(match.group('m'))
        day = int(match.group('d')) if match.group('d') else None
        era = era_of(None, others)
        if not 1 <= month <= 12:
            return {'kind': 'free'}
        certainty = certainty_of(value, others)
        if era is None and year < 1000 and antiquity:
            return {'kind': 'era', 'pending': [year, month, day, certainty]}
        return point(year, era, month, day, certainty)
    match = YEAR.match(value_core)
    if match:
        era = era_of(match.group('era'), others)
        year = int(match.group('y'))
        certainty = certainty_of(value, others, bool(match.group('approx')), bool(match.group('q')))
        if era is None and year < 1000 and antiquity:
            return {'kind': 'era', 'pending': [year, None, None, certainty]}
        return point(year, era, None, None, certainty)
    match = RANGE.match(value_core)
    if match:
        era_b = era_of(match.group('era'), others)
        era_a = era_of(match.group('ea'), []) if match.group('ea') else era_b
        a, b = int(match.group('a')), int(match.group('b'))
        if era_b is None and min(a, b) < 1000 and antiquity:
            return {'kind': 'era'}
        start, end = astronomical(a, era_a or 1), astronomical(b, era_b or 1)
        if start > end:
            return {'kind': 'free'}  # "425–497 до н. э." or "1846–93": not read
        return {'kind': 'span', 'start': lexeme(start), 'end': lexeme(end), 'years': [start, end],
                'certainty': certainty_of(value, others, bool(match.group('approx')), bool(match.group('qa') or match.group('qb')))}
    match = HIJRI_PAIR.match(value_core)
    if match:
        x, y = int(match.group('x')), int(match.group('y'))
        marked = bool(match.group('xa') or match.group('ya')) or bool(re.search(r'(?i)хиджр|hijri|\bAH\b', ' '.join(others)))
        fits = [(ah, ce) for ah, ce in ((x, y), (y, x)) if abs(ce - hijri_to_ce(ah)) <= 1.5]
        if len(fits) == 1 and (marked or fits[0][0] < fits[0][1]):
            year = fits[0][1]
            return {'kind': 'point', 'value': lexeme(year), 'year': year, 'hijri': fits[0][0],
                    'certainty': certainty_of(value, others)}
        return {'kind': 'free'}
    match = CENTURY.match(value_core)
    if match:
        first, last = roman(match.group('c')), roman(match.group('c2') or match.group('c'))
        if not first or not last or first > 21 or last > 21:
            return {'kind': 'free'}
        era = era_of(match.group('era'), others) or 1
        if era == 1:
            if first > last:
                return {'kind': 'free'}
            start, end = (first - 1) * 100 + 1, last * 100
        else:
            if first < last:
                return {'kind': 'free'}
            start, end = 1 - first * 100, -(last - 1) * 100
        return {'kind': 'span', 'start': lexeme(start), 'end': lexeme(end), 'years': [start, end], 'century': True,
                'certainty': certainty_of(value, others, approx=True)}
    return {'kind': 'free'}


def life_of(text, antiquity):
    life = {}
    gaps = set()
    status = []
    check_dates = set(CHECK_DATE.findall(text))
    for kind, words, value, others, line in life_entries(text):
        if kind == STATUS:
            status.append(line)
            check_dates.update(re.findall(r'\d{4}-\d{2}-\d{2}', line))
    for kind, words, value, others, line in life_entries(text):
        if kind == STATUS:
            continue
        if kind is None:
            if value is not None or words:
                life.setdefault('other', []).append(line)
            continue
        parsed = read_value(value, others, antiquity)
        if parsed['kind'] == 'point' and kind in (BIRTH, DEATH) and (parsed['value'] in check_dates or parsed['year'] > 2026):
            parsed = {'kind': 'check-date'}
        parsed['line'] = line
        if value:
            parsed['text'] = clean(value)
        if words:
            parsed['words'] = clean(words)
        weak = ('unchecked', 'na', 'free', 'era', 'check-date')
        if kind in life and life[kind]['kind'] not in weak:
            if parsed['kind'] in ('point', 'span') and {k: v for k, v in parsed.items() if k not in ('line', 'words')} != {k: v for k, v in life[kind].items() if k not in ('line', 'words')}:
                gaps.add(f'life:{kind}:twice')
            continue
        if kind in life and parsed['kind'] in weak:
            continue
        life[kind] = parsed
    for kind, other in ((BIRTH, DEATH), (DEATH, BIRTH)):
        entry, known = life.get(kind), life.get(other)
        if not (entry and known and entry['kind'] == 'era' and entry.get('pending') and known['kind'] == 'point' and known.get('era')):
            continue
        year, month, day, certainty = entry['pending']
        filled = point(year, known['era'], month, day, certainty)
        birth_year, death_year = (filled['year'], known['year']) if kind == BIRTH else (known['year'], filled['year'])
        if 0 <= death_year - birth_year <= 120:
            filled.pop('era')
            filled.update(line=entry['line'], era_from=other)
            if entry.get('text'):
                filled['text'] = entry['text']
            if entry.get('words'):
                filled['words'] = entry['words']
            life[kind] = filled
    for entry in life.values():
        if isinstance(entry, dict):
            entry.pop('pending', None)
    if status:
        life['status'] = status
    for kind in (BIRTH, DEATH, LIFE, ACTIVITY):
        entry = life.get(kind)
        if not entry:
            continue
        if entry['kind'] in ('free', 'era', 'check-date'):
            gaps.add(f'life:{kind}:{entry["kind"]}')
        elif entry['kind'] == 'unchecked':
            gaps.add(f'life:{kind}:unchecked')
        elif kind in (BIRTH, DEATH) and entry['kind'] == 'span':
            gaps.add(f'life:{kind}:range')
    birth, death = life.get(BIRTH, {}), life.get(DEATH, {})
    if birth.get('kind') == 'point' and death.get('kind') == 'point' and birth['year'] > death['year']:
        gaps.add('life:order')
    if life.get('other'):
        gaps.add('life:other')
    return life, sorted(gaps)


# ---------------------------------------------------------------- one row

def parse_row(batch, row):
    preferred = clean(row.get('preferred_label', ''))
    ru, verse = ru_name(row.get('label_ru', ''))
    forms = []
    if preferred:
        forms.append({'text': preferred, 'script': script_of(preferred), 'lang': None, 'from': 'preferred_label'})
    if ru and ru != preferred:
        forms.append({'text': ru, 'script': script_of(ru), 'lang': 'ru', 'from': 'label_ru'})
    seen = {form['text'] for form in forms}
    for form in variant_forms(row.get('variant_labels', '')):
        if form['text'] in seen:
            continue
        seen.add(form['text'])
        forms.append({'text': form['text'], 'script': script_of(form['text']), 'lang': form['lang'], 'from': 'variant_labels'})
    latin = next((form['text'] for form in forms if form['script'] == 'Latn'), None)
    original = next((form['text'] for form in forms if form['script'] not in (None, 'Latn', 'Cyrl')), None)
    kind, basis = kind_of(row)
    life, gaps = life_of(row.get('life_dates', ''), batch.startswith('A'))
    if latin is None:
        gaps.append('name:latin')
    ids = identifiers(row.get('external_identifiers', ''))
    if any(item.get('valid') is False for item in ids):
        gaps.append('ids:checksum')
    existing = [clean(part) for part in re.split(r'[;\n]', row.get('existing_tos_id', '')) if clean(part)]
    record = {
        'row': f'{batch}:{row["record_id"].strip()}',
        'sheet_row': row['row'],
        'names': {'ru': ru or preferred, 'latin': latin, 'original': original},
        'forms': forms,
        'kind': kind,
        'kind_basis': basis,
        'signals': signals_of(row, kind),
        'record_type': clean(row.get('record_type', '')),
        'status': clean(row.get('identity_status', '')),
        'role': clean(row.get('scope_role', '')),
        'existing': existing,
        'ids': ids,
        'life': life,
        'gaps': sorted(set(gaps)),
    }
    if verse:
        record['verse'] = verse
    return record


def build(review):
    registry = os.path.join(review, 'registry')
    if not os.path.isdir(registry):
        raise SystemExit('parse: no registry/ (run intake.py, or fetch the layers)')
    files = {}
    for batch in sorted(os.listdir(registry)):
        path = os.path.join(registry, batch, 'agents.jsonl')
        if not os.path.exists(path):
            continue
        lines = open(path, encoding='utf-8').read().splitlines()
        head = json.loads(lines[0])
        if head.get('sheet') != 'agents':
            raise SystemExit(f'parse: {path}: not an agents sheet')
        records = [parse_row(batch, json.loads(line)) for line in lines[1:]]
        files[f'layer/agents/{batch}.jsonl'] = ''.join(json.dumps(item, ensure_ascii=False) + '\n' for item in records)
    if not files:
        raise SystemExit('parse: registry/ holds no agents sheet')
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--review', default=HERE, help='the review folder (default: beside this file)')
    parser.add_argument('--check', action='store_true', help='compare with the files on disk, write nothing')
    args = parser.parse_args()
    files = build(args.review)
    differ = []
    for rel, text in files.items():
        path = os.path.join(args.review, rel)
        if args.check:
            if not os.path.exists(path) or open(path, encoding='utf-8').read() != text:
                differ.append(rel)
            continue
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path + '.tmp', 'w', encoding='utf-8', newline='\n') as handle:
            handle.write(text)
        os.replace(path + '.tmp', path)
    rows = sum(text.count('\n') for text in files.values())
    if args.check:
        if differ:
            print(f'parse: {len(differ)} files differ from what the registry gives: {", ".join(differ[:8])}')
            sys.exit(1)
        print(f'parse: matches what the registry gives ({len(files)} batches, {rows} rows)')
    else:
        print(f'parse: wrote {len(files)} batches, {rows} rows')


if __name__ == '__main__':
    main()
