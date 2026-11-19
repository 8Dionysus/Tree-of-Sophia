#!/usr/bin/env python3
"""The first batch of the Aesthetics research as one layer.

Four research registers (areas 1–12, 13–20, 21–28, 29–36) were made by separate
passes, each with its own columns, status words and local IDs. This tool reads
their workbooks and writes

- ``registry/<A|B|C|D>/<sheet>.jsonl``: every row of every content sheet as it
  stands in its workbook, one JSON object per line, under its own column names;
- ``layer/*.json``: the same content in one shape, with keys that stay unique
  across the registers, the same author and the same work joined into one
  record, and the corrections the media pass found applied and recorded;
- ``provenance.json``: the workbooks read, with their sizes and SHA-256.

    python3 bonsai/review/aesthetics/unify.py --batch "<folder of the first batch>"

Nothing here is admitted to the bonsai. A reading enters ``bonsai/aesthetics/``
only after its place has been read independently in the work itself.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import re
import sys
import unicodedata
import warnings
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent

# The registers by the group of areas each covers, and the workbook of the media pass made over each.
REGISTERS = {
    'A': {'areas': range(1, 13), 'workbook': 'Estetika_1-12_Reestr.xlsx', 'media': 'media/m_1-12.xlsx'},
    'B': {'areas': range(13, 21), 'workbook': 'Estetika_13-20_Reestr.xlsx', 'media': 'media/m_13-20.xlsx'},
    'C': {'areas': range(21, 29), 'workbook': 'Estetika_21-28_Reestr.xlsx', 'media': 'media/m_21-28.xlsx'},
    'D': {'areas': range(29, 37), 'workbook': 'Estetika_29-36_Reestr.xlsx', 'media': 'media/m_29-36.xlsx'},
}

# Content sheets by the name each register gives them. Cover sheets («Начало», «Обзор») hold counts only.
SHEETS = {
    'Области': 'areas', 'Покрытие 89': 'coverage', 'Покрытие': 'coverage',
    'Концепты': 'concepts', 'Понятия': 'concepts', 'Авторы': 'agents', 'Произведения': 'works',
    'Трактовки': 'readings', 'Связи': 'links', 'Издания': 'editions', 'Источники': 'sources',
    'Библиография': 'bibliography', 'Различения': 'distinctions', 'Материалы': 'materials',
    'Поиск': 'searches', 'Пробелы': 'gaps', 'Участие': 'memberships', 'Свидетельства': 'evidence',
    'Исторические узлы': 'historical-nodes', 'Методика': 'method',
}
COVERS = {'Начало', 'Обзор'}


def cell(value) -> str:
    """A cell as text: dates in ISO form, whole numbers without a fraction, the rest trimmed."""
    if value is None:
        return ''
    if isinstance(value, datetime.datetime):
        return value.date().isoformat() if value.time() == datetime.time(0) else value.isoformat(sep=' ')
    if isinstance(value, datetime.date):
        return value.isoformat()
    if isinstance(value, float) and value.is_integer():
        return str(int(value))
    return str(value).strip()


def read_sheet(workbook, name: str, min_cells: int = 3):
    """Rows of one sheet under its header: the first row with at least ``min_cells`` filled cells whose
    next filled row is as wide. Returns ``(header, [(row number, {column: text})])``."""
    rows = [(number, [cell(value) for value in row]) for number, row in enumerate(workbook[name].iter_rows(values_only=True), 1)]
    rows = [(number, values) for number, values in rows if any(values)]
    start = None
    for index, (_, values) in enumerate(rows[:-1]):
        filled = sum(1 for value in values if value)
        after = sum(1 for value in rows[index + 1][1] if value)
        if filled >= min_cells and after >= min_cells - 1 and len(set(v for v in values if v)) == filled:
            start = index
            break
    if start is None:
        return [], []
    header = rows[start][1]
    while header and not header[-1]:
        header = header[:-1]
    out = []
    for number, values in rows[start + 1:]:
        record = {column: value for column, value in zip(header, values) if column and value}
        if record:
            out.append((number, record))
    return [column for column in header if column], out


def read_register(batch: Path, letter: str):
    """Every content sheet of one register: ``{sheet id: (header, rows)}``."""
    import openpyxl
    warnings.filterwarnings('ignore')
    path = batch / REGISTERS[letter]['workbook']
    workbook = openpyxl.load_workbook(path, read_only=True, data_only=True)
    sheets = {}
    for name in workbook.sheetnames:
        if name in COVERS:
            continue
        if name not in SHEETS:
            raise SystemExit(f'{path.name}: sheet «{name}» is not known to this tool')
        sheets[SHEETS[name]] = (name, *read_sheet(workbook, name))
    workbook.close()
    return sheets


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=1) + '\n', encoding='utf-8')


def write_rows(path: Path, sheet: str, header, rows) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [json.dumps({'sheet': sheet, 'columns': header}, ensure_ascii=False)]
    lines += [json.dumps({'row': number, **record}, ensure_ascii=False) for number, record in rows]
    path.write_text('\n'.join(lines) + '\n', encoding='utf-8')



# ---------------------------------------------------------------------------------------------
# One shape for the four registers
# ---------------------------------------------------------------------------------------------

BASIS = {
    'P': 'primary', 'S': 'secondary', 'A': 'abstract', 'B': 'bibliography',
    'Первичный текст': 'primary', 'Вторичный источник': 'secondary', 'Только аннотация': 'abstract',
    'подтверждено первичным': 'primary', 'подтверждено вторичным': 'secondary', 'исследовательская гипотеза': 'hypothesis',
}
# How a register says a link stands: in the named work, as the compiler's comparison, on a study of the work, or as a route of the register.
GROUNDING = {
    'атрибутированная связь': 'work', 'Указано в источнике': 'work', 'текстуальное': 'work',
    'исследовательское сопоставление': 'compiler', 'Сопоставление составителя': 'compiler',
    'вторичное свидетельство': 'study', 'редакторский маршрут': 'route',
}
CYRILLIC = dict(zip('абвгдеёжзийклмнопрстуфхцчшщъыьэюя',
                    ['a', 'b', 'v', 'g', 'd', 'e', 'e', 'zh', 'z', 'i', 'y', 'k', 'l', 'm', 'n', 'o', 'p', 'r', 's', 't', 'u', 'f', 'kh', 'ts', 'ch', 'sh', 'shch', '', 'y', '', 'e', 'yu', 'ya']))
LATIN = {'ä': 'ae', 'ö': 'oe', 'ü': 'ue', 'ß': 'ss', 'æ': 'ae', 'œ': 'oe', 'ø': 'o', 'ł': 'l', 'đ': 'd'}
# Words a shortened title does not end on.
TRAILING = {'a', 'an', 'the', 'of', 'and', 'or', 'in', 'on', 'to', 'into', 'as', 'for', 'with', 'from', 'by', 'at', 'der', 'die', 'das', 'des', 'dem', 'den', 'und', 'von', 'einer', 'eines', 'in', 'zur', 'zum', 'de', 'la', 'le', 'les', 'du', 'et', 'des', 'e', 'i', 'k', 'v'}
NON_LATIN = re.compile(r'[Ͱ-Ͽἀ-῿Ѐ-ӿ֐-ۿऀ-ॿ぀-ヿ一-鿿]')


def split(text: str) -> list[str]:
    """The entries of a cell that lists several, separated by semicolons."""
    return [part.strip() for part in (text or '').split(';') if part.strip()]


def fold(text: str) -> str:
    """A name or title as compared: marks and punctuation dropped, case folded."""
    text = unicodedata.normalize('NFKD', text.replace('ё', 'е').replace('Ё', 'Е'))
    text = ''.join(char for char in text if not unicodedata.combining(char))
    return re.sub(r'\s+', ' ', re.sub(r'[^\w\s]', ' ', text.lower())).strip()


def slug(text: str, limit: int = 48) -> str:
    """An identifier from a name or title, in the Tree's habit: German umlauts spelled out, other marks dropped."""
    text = ''.join(CYRILLIC.get(char, char) for char in text.lower())
    text = ''.join(LATIN.get(char, char) for char in text)
    text = unicodedata.normalize('NFKD', text)
    text = ''.join(char for char in text if not unicodedata.combining(char))
    text = re.sub(r'[’\'`ʼ]s\b', 's', text)
    text = re.sub(r'[^a-z0-9]+', '-', text).strip('-')
    if len(text) > limit:
        text = text[:limit + 1].rsplit('-', 1)[0]
        while text.rsplit('-', 1)[-1] in TRAILING and '-' in text:
            text = text.rsplit('-', 1)[0]
    return text


def latin_part(text: str) -> str:
    """The first Latin-script form among the forms a cell gives («Περὶ ποιητικῆς / Poetics»)."""
    for part in (piece.strip() for piece in text.split(' / ')):
        if part and not NON_LATIN.search(part) and re.search(r'[A-Za-z]', part):
            return part
    return ''


def year_of(text: str):
    match = re.search(r'(?<!\d)(\d{4})(?!\d)', text or '')
    return int(match.group(1)) if match and not re.search(r'до н\.?\s*э', text) else None


def area_ids(text: str) -> list[str]:
    return [f'AE{int(number):02d}' for number in re.findall(r'\d+', text or '')]


class Groups:
    """Records joined into one: a union-find over register keys."""

    def __init__(self, keys):
        self.parent = {key: key for key in keys}

    def find(self, key):
        while self.parent[key] != key:
            self.parent[key] = self.parent[self.parent[key]]
            key = self.parent[key]
        return key

    def join(self, a, b):
        a, b = self.find(a), self.find(b)
        if a != b:
            first, second = sorted((a, b))
            self.parent[second] = first

    def groups(self):
        out = defaultdict(list)
        for key in self.parent:
            out[self.find(key)].append(key)
        return {root: sorted(keys) for root, keys in out.items()}


def words(text: str) -> dict:
    """A register's own words under their language: most rows are Russian, some of 13–20 are English."""
    return {'ru' if re.search(r'[А-Яа-яЁё]', text) or not re.search(r'[A-Za-z]', text) else 'en': text}


def pick(record: dict, *columns: str) -> str:
    for column in columns:
        if record.get(column):
            return record[column]
    return ''


def unify(registers: dict, media: dict, inputs: dict) -> dict:
    """The layer: the registers' content in one shape. ``media`` holds what the media pass corrected."""
    corrections = []
    rows = lambda letter, sheet: registers[letter].get(sheet, ('', [], []))[2]
    key = lambda letter, identifier: f'{letter}:{identifier}'

    # A register may name one record by several IDs; each resolves to the row that holds it.
    alias = {}
    for letter in 'BD':
        for sheet in ('agents', 'works', 'sources'):
            for _, record in rows(letter, sheet):
                for other in split(record.get('Другие ID', '')):
                    alias[key(letter, other)] = key(letter, record['ID'])
        for _, record in rows(letter, 'concepts'):
            for other in split(record.get('Исходные ID', '')):
                alias[key(letter, other)] = key(letter, record['ID'])
    for record in media['B']['same']:
        alias.setdefault(key('B', record['Альтернативный ID']), key('B', record['Канонический ID']))
    resolve = lambda letter, identifier: alias.get(key(letter, identifier), key(letter, identifier))

    # --- the media pass's corrections of the registers' own fields --------------------------------
    fixed = {}
    for record in media['B']['fixes']:
        target = resolve('B', record['Исходный ID'])
        fixed[target] = record
        corrections.append({
            'target': target, 'kind': record['Тип цели'], 'field': record['Поле с ошибкой'],
            'was': record['Значение в исходном реестре'], 'now': record['Значение в первоисточнике'],
            **({'name_ru': record['Проверенное имя по-русски']} if record.get('Проверенное имя по-русски') else {}),
            'verified_at': [url for url in (record.get('Первоисточник проверки'), record.get('Дополнительное подтверждение')) if url],
            'statement': record.get('Что подтверждено', ''), 'by': 'media pass over 13–20, sheet «Исправления»', 'applied': True,
        })
    remarks = defaultdict(list)
    for record in media['A']['remarks']:
        target = key('A', record['target_id'])
        remarks[target].append({'found': record['Что выявлено'], 'register': record.get('Исходная запись', ''), 'checked': record.get('Проверенный результат', ''), 'evidence': record.get('Свидетельство', ''), 'kept': record.get('Как учтено', '')})
        corrections.append({'target': target, 'kind': 'remark', 'field': record['Что выявлено'], 'was': record.get('Исходная запись', ''), 'now': record.get('Проверенный результат', ''), 'verified_at': split(record.get('Свидетельство', '')), 'statement': record.get('Как учтено', ''), 'by': 'media pass over 1–12, sheet «Замечания»', 'applied': False})
    for letter, sheets in media['later'].items():
        for sheet, changes in sheets.items():
            for identifier, column, was, now in changes:
                corrections.append({'target': key(letter, identifier), 'kind': sheet, 'field': column, 'was': was, 'now': now, 'verified_at': [], 'statement': '', 'by': f'media pass over {"21–28" if letter == "C" else "29–36"}: the register sheet as its workbook holds it', 'applied': True})

    # --- sources -------------------------------------------------------------------------------
    sources = {}
    for letter in 'ABCD':
        for number, record in rows(letter, 'sources'):
            identifier = pick(record, 'Источник ID', 'ID')
            sources[key(letter, identifier)] = {
                'key': key(letter, identifier), 'row': number,
                'title': pick(record, 'Название источника', 'Издание / ресурс', 'Источник / точка доступа'),
                'edition': record.get('Издание / библиографические данные', ''),
                'url': pick(record, 'URL', 'Полный URL'),
                'kind': record.get('Тип источника', ''), 'language': record.get('Язык', ''),
                'access': pick(record, 'Статус доступа', 'Доступ'),
                'read': pick(record, 'Что проверено', 'Проверенные места', 'Фактически проверенный объём'),
                'find': record.get('Как искать место', ''),
                'checked_on': pick(record, 'Дата проверки', 'Проверено'),
                'note': pick(record, 'Оговорка', 'Примечание к источнику', 'Примечание'),
                'works': [resolve(letter, item) for item in split(pick(record, 'Произведения ID', 'ID произведения'))],
                **({'also': sorted(k for k, v in alias.items() if v == key(letter, identifier) and k != v)} if any(v == key(letter, identifier) and k != v for k, v in alias.items()) else {}),
            }
    source_ref = lambda letter, text: [resolve(letter, item) for item in split(text)]

    # --- agents --------------------------------------------------------------------------------
    raw_agents = {}
    for letter in 'ABCD':
        for number, record in rows(letter, 'agents'):
            identifier = pick(record, 'Автор ID', 'ID')
            entry = {
                'key': key(letter, identifier), 'row': number,
                'ru': pick(record, 'Имя по-русски', 'Имя'), 'original': pick(record, 'Оригинальное имя', 'Имя в источнике'),
                'dates': pick(record, 'Даты', 'Даты / период'), 'tradition': pick(record, 'Традиция / область', 'Контекст / традиция'),
                'role': pick(record, 'Роль', 'Роль в реестре'), 'languages': record.get('Языки', ''), 'note': record.get('Примечание', ''),
            }
            fix = fixed.get(entry['key'])
            if fix and fix['Тип цели'] == 'author':
                entry['register_name'] = {'ru': entry['ru'], 'original': entry['original']}
                entry['ru'], entry['original'] = fix['Проверенное имя по-русски'], fix['Значение в первоисточнике']
            raw_agents[entry['key']] = entry
    agent_groups = Groups(raw_agents)
    # One person across registers, and within one where its corrected name meets another record: the same name, original or Russian.
    for index in (lambda entry: fold(entry['original']), lambda entry: fold(entry['ru'])):
        seen = {}
        for entry_key, entry in sorted(raw_agents.items()):
            agent_groups.join(seen.setdefault(index(entry), entry_key), entry_key)
    for same in inputs['agents']['same']:
        for other in same['keys'][1:]:
            agent_groups.join(same['keys'][0], other)
    agents, agent_of = {}, {}
    for root, members in sorted(agent_groups.groups().items()):
        entries = [raw_agents[member] for member in members]
        ru_names = Counter(entry['ru'] for entry in entries)
        manual = next((inputs['agents']['names'][name] for name in ru_names if name in inputs['agents']['names']), None)
        chosen = next((same['name'] for same in inputs['agents']['same'] if same['keys'][0] in members), None)
        latin = Counter(latin_part(entry['original']) for entry in entries if latin_part(entry['original']))
        en = manual['en'] if manual else chosen['en'] if chosen else latin.most_common(1)[0][0] if latin else ''
        ru = chosen['ru'] if chosen else ru_names.most_common(1)[0][0]
        identifier = manual['id'] if manual else slug(en)
        if not identifier:
            raise SystemExit(f'agent {members}: no Latin-script name; name it in inputs/agents.json')
        if identifier in agents:
            raise SystemExit(f'agent {identifier}: {members} and {agents[identifier]["keys"]} take one ID; decide in inputs/agents.json')
        original = next((entry['original'].split(' / ')[0].strip() for entry in entries if NON_LATIN.search(entry['original'].split(' / ')[0])), '')
        agents[identifier] = {
            'id': identifier, 'keys': members, 'name': {'ru': ru, 'en': en, **({'original': original} if original else {})},
            **({'names': sorted({entry['ru'] for entry in entries} | {entry['original'] for entry in entries} - {ru, en})} if len({entry['ru'] for entry in entries} | {entry['original'] for entry in entries}) > 2 else {}),
            'dates': sorted({entry['dates'] for entry in entries if entry['dates']}),
            'tradition': sorted({entry['tradition'] for entry in entries if entry['tradition']}),
            'roles': sorted({entry['role'] for entry in entries if entry['role']}),
            'notes': sorted({entry['note'] for entry in entries if entry['note']}),
            **({'register_names': [dict(key=entry['key'], **entry['register_name']) for entry in entries if 'register_name' in entry]} if any('register_name' in entry for entry in entries) else {}),
            **({'remarks': [remark for member in members for remark in remarks.get(member, [])]} if any(member in remarks for member in members) else {}),
        }
        for member in members:
            agent_of[member] = identifier
    for alternative, canonical in alias.items():
        if canonical in agent_of:
            agent_of.setdefault(alternative, agent_of[canonical])
    by_name = defaultdict(dict)
    for entry in raw_agents.values():
        letter = entry['key'][0]
        for name in {entry['ru'], entry.get('register_name', {}).get('ru', entry['ru'])}:
            by_name[letter][fold(name)] = agent_of[entry['key']]

    def agents_named(letter: str, text: str) -> tuple[list[str], list[str]]:
        """The agents a cell names in words, and the names it gives that no author record holds."""
        found, unknown = [], []
        for name in (part.strip() for part in re.split(r';', text or '')):
            if not name:
                continue
            identifier = by_name[letter].get(fold(name))
            if identifier is None:
                parts = [by_name[letter].get(fold(piece)) for piece in re.split(r',| и ', name) if piece.strip()]
                if parts and all(parts):
                    found += [part for part in parts if part not in found]
                    continue
                unknown.append(name)
            elif identifier not in found:
                found.append(identifier)
        return found, unknown

    # --- works ---------------------------------------------------------------------------------
    raw_works = {}
    for letter in 'ABCD':
        for number, record in rows(letter, 'works'):
            identifier = pick(record, 'Работа ID', 'ID')
            entry = {
                'key': key(letter, identifier), 'row': number,
                'ru': pick(record, 'Русское название', 'Название по-русски'), 'original': pick(record, 'Оригинальное заглавие', 'Оригинальное название'),
                'date': pick(record, 'Дата', 'Дата / издание', 'Дата / версия'), 'dating': record.get('Тип датировки', ''),
                'kind': pick(record, 'Тип работы', 'Вид работы', 'Тип произведения'), 'language': record.get('Язык', ''),
                'access': BASIS.get(record.get('Уровень доступа', ''), ''), 'locus': record.get('Проверенный локатор', ''),
                'note': pick(record, 'Примечание', 'Библиографическое примечание'),
                'sources': source_ref(letter, pick(record, 'Источники ID', 'ID доступных источников')),
                'areas': area_ids(record.get('Области', '')),
            }
            ids = [agent_of.get(resolve(letter, item)) for item in split(record.get('Авторы ID', ''))]
            names, unknown = agents_named(letter, pick(record, 'Автор / авторы', 'Полный состав авторов'))
            entry['agents'] = [item for item in ids if item] or names
            if unknown and not ids:
                entry['authors_unresolved'] = unknown
            fix = fixed.get(entry['key'])
            if fix and fix['Тип цели'] == 'work':
                entry['register_title'] = entry['original']
                entry['original'] = fix['Значение в первоисточнике']
            raw_works[entry['key']] = entry
    work_groups = Groups(raw_works)
    seen_original, seen_ru = {}, defaultdict(list)
    for entry_key, entry in sorted(raw_works.items()):
        first = entry['agents'][0] if entry['agents'] else ''
        forms = {fold(part) for part in entry['original'].split(' / ') if part.strip()}
        # Two records of one register are two works unless the register itself joins them; across registers the
        # same author's work is one when a form of its original title agrees, or its Russian title and its year.
        for form in forms:
            other_key = seen_original.setdefault((first, form), entry_key)
            if other_key[0] != entry_key[0]:
                work_groups.join(other_key, entry_key)
        for other_key, other_year, other_forms in seen_ru[(first, fold(entry['ru']))]:
            if other_key[0] != entry_key[0] and (other_year == year_of(entry['date']) or other_forms & forms):
                work_groups.join(other_key, entry_key)
        seen_ru[(first, fold(entry['ru']))].append((entry_key, year_of(entry['date']), forms))
    for same in inputs['works']['same']:
        for other in same['keys'][1:]:
            work_groups.join(same['keys'][0], other)
    works, work_of = {}, {}
    for root, members in sorted(work_groups.groups().items()):
        entries = [raw_works[member] for member in members]
        manual = next((inputs['works']['titles'][member] for member in members if member in inputs['works']['titles']), None)
        work_agents = []
        for entry in entries:
            work_agents += [agent for agent in entry['agents'] if agent not in work_agents]
        latin = Counter(latin_part(entry['original']) for entry in entries if latin_part(entry['original']))
        title = latin.most_common(1)[0][0] if latin else ''
        # A register may write the author's name before the title («John Morreall. Philosophy of Humor»).
        for agent in work_agents:
            name = agents[agent]['name']['en']
            if name and title.startswith(name + '. '):
                title = title[len(name) + 2:]
        short = re.split(r':| — |\. ', title)[0] if title else ''
        tail = manual['id'] if manual else slug(short if len(short.split()) > 1 or not title else short or title)
        if not tail:
            raise SystemExit(f'work {members} «{entries[0]["ru"]}»: no Latin-script title; name it in inputs/works.json')
        identifier = f'{work_agents[0] if work_agents else "anonymous"}.{tail}'
        if identifier in works:
            year = next((year_of(entry['date']) for entry in entries if year_of(entry['date'])), None)
            identifier = f'{identifier}-{year}' if year and f'{identifier}-{year}' not in works else None
            if identifier is None:
                raise SystemExit(f'work {members} «{entries[0]["ru"]}» takes the ID of another work; decide in inputs/works.json')
        non_latin = next((entry['original'].split(' / ')[0].strip() for entry in entries if NON_LATIN.search(entry['original'].split(' / ')[0])), '')
        works[identifier] = {
            'id': identifier, 'keys': members, 'agents': work_agents,
            'title': {'ru': Counter(entry['ru'] for entry in entries).most_common(1)[0][0], **({'en': manual['en']} if manual and manual.get('en') else {}), 'original': non_latin or title or entries[0]['original']},
            'titles': sorted({entry['ru'] for entry in entries} | {entry['original'] for entry in entries}),
            'dates': sorted({entry['date'] for entry in entries if entry['date']}),
            'year': next((year_of(entry['date']) for entry in entries if year_of(entry['date'])), None),
            'kinds': sorted({entry['kind'] for entry in entries if entry['kind']}),
            'languages': sorted({entry['language'] for entry in entries if entry['language']}),
            'access': sorted({entry['access'] for entry in entries if entry['access']}),
            'sources': sorted({source for entry in entries for source in entry['sources']}),
            'notes': sorted({entry['note'] for entry in entries if entry['note']}),
            **({'register_titles': [dict(key=entry['key'], original=entry['register_title']) for entry in entries if 'register_title' in entry]} if any('register_title' in entry for entry in entries) else {}),
            **({'authors_unresolved': sorted({name for entry in entries for name in entry.get('authors_unresolved', [])})} if any(entry.get('authors_unresolved') for entry in entries) else {}),
            **({'remarks': [remark for member in members for remark in remarks.get(member, [])]} if any(member in remarks for member in members) else {}),
        }
        for member in members:
            work_of[member] = identifier
    for alternative, canonical in alias.items():
        if canonical in work_of:
            work_of.setdefault(alternative, work_of[canonical])

    # --- concepts ------------------------------------------------------------------------------
    raw_concepts = {}
    for letter in 'ABCD':
        for number, record in rows(letter, 'concepts'):
            identifier = pick(record, 'Узел ID', 'ID')
            raw_concepts[key(letter, identifier)] = {
                'key': key(letter, identifier), 'row': number, 'ru': pick(record, 'Название', 'Понятие'),
                'terms': split(pick(record, 'Оригинальные термины', 'Термины в других языках')),
                'kind': record.get('Тип узла', ''),
                'areas': area_ids(' '.join([record.get('Основная область', ''), record.get('Другие области', ''), record.get('Области', ''), record.get('Области исходного списка', '')])),
                'content': pick(record, 'Рабочее содержание', 'Рабочее определение', 'Рабочая область значения'),
                'boundary': pick(record, 'Границы и оговорки', 'Граница / различение', 'Что необходимо различать'),
                'basis': BASIS.get(record.get('Уровень', ''), record.get('Уровень', '')), 'note': record.get('Примечание', ''),
                'asked_as': record.get('Термин из запроса', ''),
            }
    concept_groups = Groups(raw_concepts)
    seen = {}
    for entry_key, entry in sorted(raw_concepts.items()):
        concept_groups.join(seen.setdefault(fold(entry['ru']), entry_key), entry_key)
    concepts, concept_of = {}, {}
    for root, members in sorted(concept_groups.groups().items()):
        entries = [raw_concepts[member] for member in members]
        terms, areas = [], []
        for entry in entries:
            terms += [term for term in entry['terms'] if term not in terms]
            areas += [area for area in entry['areas'] if area not in areas]
        concepts[root] = {
            'key': root, 'keys': members, 'title': {'ru': entries[0]['ru']}, 'terms': terms, 'areas': sorted(areas),
            'kinds': sorted({entry['kind'] for entry in entries if entry['kind']}),
            'said': [{'key': entry['key'], 'content': words(entry['content']), **({'boundary': words(entry['boundary'])} if entry['boundary'] else {}), **({'basis': entry['basis']} if entry['basis'] else {}), **({'note': entry['note']} if entry['note'] else {})} for entry in entries],
        }
        for member in members:
            concept_of[member] = root
    for alternative, canonical in alias.items():
        if canonical in concept_of:
            concept_of.setdefault(alternative, concept_of[canonical])

    # --- readings ------------------------------------------------------------------------------
    evidence = defaultdict(list)
    for number, record in rows('C', 'evidence'):
        for reading in split(record.get('ID трактовок', '')):
            evidence[key('C', reading)].append({
                'key': key('C', record['ID']), 'source': resolve('C', record.get('ID источника', '')), 'locus': record.get('Точный локус', ''),
                'system': record.get('Система локализации', ''), 'checked': record.get('Что проверено', ''), 'summary': record.get('Краткий пересказ места', ''),
                **({'quote': record['Короткая цитата']} if record.get('Короткая цитата') else {}), **({'note': record['Примечание']} if record.get('Примечание') else {}), 'url': record.get('URL', ''),
            })
    readings, unresolved = {}, []
    for letter in 'ABCD':
        for number, record in rows(letter, 'readings'):
            identifier = pick(record, 'Трактовка ID', 'ID')
            this = key(letter, identifier)
            work_key = resolve(letter, pick(record, 'Работа ID', 'Произведение ID', 'ID произведения'))
            work = work_of.get(work_key)
            if letter == 'C':
                reading_agents = [agent_of[resolve('C', item)] for item in split(record.get('ID авторов', ''))]
                unknown = []
            else:
                named = pick(record, 'Автор основания', 'Автор / авторы')
                reading_agents, unknown = agents_named(letter, re.sub(r';?\s*и ещё \d+ соавт\.?', '', named))
                # «и ещё 4 соавт.»: the work's own record lists them all.
                # An empty cell, like one without a named author, is answered by the work's own record.
                if ('соавт' in named or not named) and work:
                    reading_agents = list(works[work]['agents'])
            if work is None or unknown:
                unresolved.append((this, work_key if work is None else '', unknown))
            concept_keys = [concept_of.get(resolve(letter, item)) for item in split(pick(record, 'Узлы ID', 'Понятия ID', 'ID понятия'))]
            reading_sources = source_ref(letter, pick(record, 'Источники ID', 'Источник ID', 'ID источников'))
            entry = {
                'key': this, 'row': number,
                'areas': area_ids(pick(record, 'Области', 'Область')),
                'concepts': [item for item in concept_keys if item],
                'work': work, 'agents': reading_agents,
                'locus': pick(record, 'Локатор', 'Конкретное место'),
                'thesis': words(pick(record, 'Содержание', 'Тезис источника', 'Трактовка')),
            }
            if letter in 'BD':
                entry['reading'] = words(record.get('Трактовка / значение', ''))
                if record.get('Исходные термины'):
                    entry['terms'] = record['Исходные термины']
            if letter == 'C':
                entry['argument'] = words(record.get('Основание / аргумент', ''))
            boundary = pick(record, 'Оговорка', 'Что важно различать', 'Уточнение и границы')
            if boundary:
                entry['distinction'] = words(boundary)
            entry['basis'] = BASIS[pick(record, 'Уровень', 'Основание проверки', 'Статус основания')]
            entry['sources'] = reading_sources
            entry['url'] = pick(record, 'Основной URL', 'URL источника') or next((sources[source]['url'] for source in reading_sources if source in sources and sources[source]['url']), '')
            if record.get('Доступ к тексту'):
                entry['access'] = record['Доступ к тексту']
            if record.get('Примечание'):
                entry['note'] = record['Примечание']
            if this in evidence:
                entry['evidence'] = evidence[this]
            fix = fixed.get(this)
            if fix and fix['Тип цели'] == 'interpretation':
                entry['register_thesis'], entry['thesis'] = entry['thesis'], words(fix['Значение в первоисточнике'])
            place = media['B']['places'].get(identifier) if letter == 'B' else None
            if place:
                entry['place_check'] = place
            if this in remarks:
                entry['remarks'] = remarks[this]
            if None in concept_keys:
                entry['concepts_unresolved'] = [item for item, found in zip(split(pick(record, 'Узлы ID', 'Понятия ID', 'ID понятия')), concept_keys) if found is None]
            if unknown:
                entry['authors_unresolved'] = unknown
            readings[this] = entry

    # --- links ---------------------------------------------------------------------------------
    def endpoint(letter: str, identifier: str):
        target = resolve(letter, identifier)
        if target in concept_of:
            return {'concept': concept_of[target]}
        if target in readings:
            return {'reading': target}
        if target in work_of:
            return {'work': work_of[target]}
        return {'unresolved': target}

    reading_of_evidence = {item['key']: reading for reading, items in evidence.items() for item in items}
    links = {}
    for letter in 'ABCD':
        for number, record in rows(letter, 'links'):
            identifier = pick(record, 'Связь ID', 'ID')
            this = key(letter, identifier)
            mode = pick(record, 'Режим связи', 'Статус связи', 'Основание связи')
            if letter == 'C':
                grounds = sorted({reading_of_evidence[key('C', item)] for item in split(record.get('ID свидетельств', '')) if key('C', item) in reading_of_evidence})
            else:
                grounds = [key(letter, item) for item in split(pick(record, 'Основания ID', 'Трактовки-основания ID'))]
            entry = {
                'key': this, 'row': number,
                'from': endpoint(letter, pick(record, 'Откуда ID', 'ID начала')), 'to': endpoint(letter, pick(record, 'Куда ID', 'ID конца')),
                'named': {'from': pick(record, 'Откуда', 'От понятия'), 'to': pick(record, 'Куда', 'К понятию')},
                'verb': pick(record, 'Отношение', 'Тип связи'),
                **({'content': words(record['Содержание связи'])} if record.get('Содержание связи') else {}),
                'grounding': GROUNDING[mode], 'mode': mode,
                'grounds': [ground for ground in grounds if ground in readings], 
                **({'grounds_unresolved': [ground for ground in grounds if ground not in readings]} if any(ground not in readings for ground in grounds) else {}),
                **({'works': [work_of[resolve(letter, item)] for item in split(record['Контекст: работы ID']) if resolve(letter, item) in work_of]} if record.get('Контекст: работы ID') else {}),
                **({'basis': BASIS[record['Уровень']]} if record.get('Уровень') in BASIS else {}),
                **({'locus': pick(record, 'Конкретное место / маршрут', 'Локусы основания')} if pick(record, 'Конкретное место / маршрут', 'Локусы основания') else {}),
                **({'note': pick(record, 'Примечание', 'Уточнение')} if pick(record, 'Примечание', 'Уточнение') else {}),
                'sources': source_ref(letter, pick(record, 'Источники ID', 'ID источников')),
            }
            fix = fixed.get(this)
            if fix and fix['Тип цели'] == 'relation':
                entry['register_locus'], entry['locus'] = fix['Значение в исходном реестре'], fix['Значение в первоисточнике']
            links[this] = entry

    # --- what only one register holds -------------------------------------------------------------
    distinctions = [{
        'key': key('A', record['ID']), 'row': number, 'first': record.get('Первый предмет', ''), 'second': record.get('Второй предмет', ''),
        'distinction': {'ru': record.get('Различение', '')}, 'grounds': [key('A', item) for item in split(record.get('Основания ID', ''))],
        'sources': source_ref('A', record.get('Источники ID', '')), 'status': record.get('Статус', ''),
    } for number, record in rows('A', 'distinctions')]
    materials = [{
        'key': key('A', record['Материал ID']), 'row': number, 'title': {'ru': record.get('Название / предмет', '')}, 'kind': record.get('Тип материала', ''),
        'work': work_of.get(key('A', record.get('Работа ID', ''))), 'locus': record.get('Локатор основания', ''),
        'readings': [key('A', item) for item in split(record.get('Трактовки ID', ''))], 'basis': BASIS.get(record.get('Уровень', ''), ''),
        'boundary': {'ru': record.get('Граница использования', '')}, 'sources': source_ref('A', record.get('Источники ID', '')),
        **({'remarks': remarks[key('A', record['Материал ID'])]} if key('A', record['Материал ID']) in remarks else {}),
    } for number, record in rows('A', 'materials')]
    editions = [{
        'key': key('A', record['Версия ID']), 'row': number, 'work': work_of.get(key('A', record.get('Работа ID', ''))), 'language': record.get('Язык', ''),
        'edition': record.get('Издание / свидетель', ''), 'dated': record.get('Дата версии', ''), 'read': record.get('Проверенный объём', ''),
        'sources': source_ref('A', record.get('Источники ID', '')), 'url': record.get('Основной URL', ''), 'note': record.get('Примечание', ''),
    } for number, record in rows('A', 'editions')]
    historical = [{
        'key': key('C', record['ID']), 'row': number, 'title': {'ru': record.get('Название', '')}, 'kind': record.get('Тип узла', ''), 'period': record.get('Период / срез', ''),
        'description': {'ru': record.get('Описание', '')}, 'concepts': [concept_of[key('C', item)] for item in split(record.get('ID понятий', '')) if key('C', item) in concept_of],
        'readings': [key('C', item) for item in split(record.get('ID трактовок', ''))], 'boundary': {'ru': record.get('Оговорка', '')},
    } for number, record in rows('C', 'historical-nodes')]

    # --- areas ---------------------------------------------------------------------------------
    stated = {record['ID области']: record for _, record in rows('A', 'areas')}
    areas = []
    for area in inputs['areas']['areas']:
        entry = dict(area)
        entry['register'] = next(letter for letter, register in REGISTERS.items() if int(area['id'][2:]) in register['areas'])
        if area['id'] in stated:
            entry['scope'] = {'ru': stated[area['id']].get('Предметный охват', '')}
            entry['boundary'] = {'ru': stated[area['id']].get('Граница', '')}
        entry['concepts'] = sum(1 for concept in concepts.values() if area['id'] in concept['areas'])
        entry['readings'] = sum(1 for reading in readings.values() if area['id'] in reading['areas'])
        areas.append(entry)

    keys = {}
    for kind, table in (('agent', agent_of), ('work', work_of), ('concept', concept_of)):
        for register_key, identifier in table.items():
            keys[register_key] = {'kind': kind, 'id': identifier}
    return {
        'areas': areas, 'concepts': list(concepts.values()), 'agents': list(agents.values()), 'works': list(works.values()),
        'readings': list(readings.values()), 'links': list(links.values()), 'sources': list(sources.values()),
        'distinctions': distinctions, 'materials': materials, 'editions': editions, 'historical-nodes': historical,
        'corrections': corrections, 'keys': dict(sorted(keys.items())), '_unresolved': unresolved,
    }


def read_media(batch: Path, registers: dict) -> dict:
    """What the media pass found in the registers themselves: corrected fields, IDs naming one record,
    remarks on places, the places it opened, and the register sheets it rewrote in its own workbook."""
    import openpyxl
    warnings.filterwarnings('ignore')
    out = {'A': {'remarks': []}, 'B': {'fixes': [], 'same': [], 'places': {}}, 'later': {}}
    book = openpyxl.load_workbook(batch / REGISTERS['A']['media'], read_only=True, data_only=True)
    out['A']['remarks'] = [record for _, record in read_sheet(book, 'Замечания')[1]]
    book.close()
    book = openpyxl.load_workbook(batch / REGISTERS['B']['media'], read_only=True, data_only=True)
    out['B']['fixes'] = [record for _, record in read_sheet(book, 'Исправления')[1]]
    out['B']['same'] = [record for _, record in read_sheet(book, 'ID_соответствия')[1]]
    for _, record in read_sheet(book, 'Места_источников')[1]:
        out['B']['places'][record['ID трактовки']] = {
            'status': record.get('Статус проверки текста', ''),
            **({'opened': record['Проверенный переход']} if record.get('Проверенный переход') else {}),
            **({'pdf_page': record['Страница PDF (с 1)']} if record.get('Страница PDF (с 1)') else {}),
            **({'edition': record['Использованное издание']} if record.get('Использованное издание') else {}),
            **({'locator_kind': record['Тип цифрового локатора']} if record.get('Тип цифрового локатора') else {}),
            'by': 'media pass over 13–20, sheet «Места_источников»',
        }
    book.close()
    # The media workbooks of 21–28 and 29–36 repeat the register's sheets; where a row differs, theirs is the later reading.
    for letter in 'CD':
        book = openpyxl.load_workbook(batch / REGISTERS[letter]['media'], read_only=True, data_only=True)
        changed = {}
        for sheet_id, (name, header, rows) in registers[letter].items():
            if name not in book.sheetnames or sheet_id in ('coverage', 'searches', 'gaps', 'method'):
                continue
            later_header, later_rows = read_sheet(book, name, 8 if letter == 'D' else 3)
            identifier = header[0]
            later = {record.get(identifier): record for _, record in later_rows}
            for index, (number, record) in enumerate(rows):
                other = later.get(record.get(identifier))
                if other is None:
                    continue
                for column in header:
                    if column in later_header and record.get(column, '') != other.get(column, ''):
                        changed.setdefault(sheet_id, []).append((record[identifier], column, record.get(column, ''), other.get(column, '')))
                        record[column] = other.get(column, '')
        book.close()
        if changed:
            out['later'][letter] = changed
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--batch', required=True, type=Path, help='folder holding the four register workbooks and media/')
    parser.add_argument('--out', type=Path, default=HERE, help='where registry/, layer/ and provenance.json are written')
    args = parser.parse_args()
    registers = {letter: read_register(args.batch, letter) for letter in REGISTERS}
    for letter, sheets in registers.items():
        for sheet_id, (name, header, rows) in sheets.items():
            write_rows(args.out / 'registry' / letter / f'{sheet_id}.jsonl', name, header, rows)
    # The media pass's later readings replace the registers' rows only after those rows are written as they stand.
    media = read_media(args.batch, registers)
    inputs = {name: json.loads((HERE / 'inputs' / f'{name}.json').read_text(encoding='utf-8')) for name in ('areas', 'agents', 'works')}
    layer = unify(registers, media, inputs)
    unresolved = layer.pop('_unresolved')
    for name, value in layer.items():
        write_json(args.out / 'layer' / f'{name}.json', value)
    workbooks = [REGISTERS[letter][kind] for letter in REGISTERS for kind in ('workbook', 'media')]
    write_json(args.out / 'provenance.json', {
        'batch': 'Aesthetics, first batch: research registers of 6–7 November 2026 and the media pass over them',
        'workbooks': [{'path': name, 'bytes': (args.batch / name).stat().st_size, 'sha256': digest(args.batch / name)} for name in workbooks],
        'read': {'registers': 'every content sheet, to registry/', 'media': 'the sheets that correct the registers: «Замечания» (1–12), «Исправления», «ID_соответствия», «Места_источников» (13–20), and the register sheets repeated in 21–28 and 29–36'},
        'layer': {name: len(value) for name, value in layer.items()},
    })
    print({name: len(value) for name, value in layer.items()})
    grounding = Counter(link['grounding'] for link in layer['links'])
    print('links by grounding', dict(grounding), '· readings by basis', dict(Counter(reading['basis'] for reading in layer['readings'])))
    for item in unresolved:
        print('unresolved', item)


if __name__ == '__main__':
    main()
