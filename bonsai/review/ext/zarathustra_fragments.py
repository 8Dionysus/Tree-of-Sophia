"""Builds bonsai/zarathustra/fragments.json (tos.bonsai.fragments.v1) from the places the external-fragments
agent fetched (ext/external-fragments.json), with the stand's work keys, locators, labels and source links.
Only the places the stand cites are kept. Usage: python3 -I zarathustra_fragments.py"""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, re, sys
S = REVIEW
REPO = REPO_ROOT
src = json.load(open(f'{S}/ext/external-fragments.json'))
cited = json.load(open(f'{S}/ext/content-locs.json'))
agent_places = {(p['work'], p['loc']): p for p in src['places']}

CHAPTER = {'ru': 'Глава «Миф о Сизифе»', 'en': 'Chapter “The Myth of Sisyphus”'}
def chapter(ru, en): return {'ru': f'{CHAPTER["ru"]}: {ru}', 'en': f'{CHAPTER["en"]}: {en}'}
CUP_NP = 'https://cup.columbia.edu/book/nietzsche-and-philosophy/9780231138772/'
CUP_DR = 'https://cup.columbia.edu/book/difference-and-repetition/9780231081597/'
DELEUZE_RIGHTS = {
    'russia': 'Делёз ум. 1995: охраняется до 1 января 2066 г. (ГК РФ ст. 1281); цитата возможна (ст. 1274 п. 1 пп. 1) только после сверки с изданием.',
    'usa': 'Protected (French publication URAA-restored, 95 years from publication); English translation protected.',
    'eu': 'Protected until the end of 2065 (70 years p.m.a.).',
    'reasoning': 'Gilles Deleuze (1925–1995). No legitimate copy of the French or English text could be consulted (Internet Archive copies are lending-only or unauthorised uploads), so no wording is shown and the place is cited by its locator alone. Page numbers are left out because they could not be verified.'}

# Stand work key -> the agent's work, the stand's names for it, its witnesses' links and short names, and its places.
WORKS = {
    'die-froehliche-wissenschaft': {
        'agent': 'gs', 'author': {'ru': 'Фридрих Ницше', 'en': 'Friedrich Nietzsche'}, 'title': {'ru': 'Весёлая наука', 'en': 'The Gay Science'}, 'original': 'de',
        'witnesses': {'de': {'url': 'https://archive.org/details/bub_gb_uIQNAAAAYAAJ', 'short': {'ru': 'изд. 1887 г.', 'en': '1887 issue'}},
                      'en': {'url': 'https://www.gutenberg.org/ebooks/52881', 'short': {'ru': 'пер. Т. Коммона, 1910', 'en': 'tr. Thomas Common, 1910'}}},
        'places': {'276': {'agent': '276', 'label': {'ru': '§276', 'en': '§276'}}, '341': {'agent': '341', 'label': {'ru': '§341', 'en': '§341'}}}},
    'ethica': {
        'agent': 'spinoza', 'author': {'ru': 'Бенедикт Спиноза', 'en': 'Benedict de Spinoza'}, 'title': {'ru': 'Этика', 'en': 'Ethics'}, 'original': 'la',
        'witnesses': {'la': {'url': 'https://la.wikisource.org/wiki/Ethica/Pars_tertia_-_De_origine_et_natura_affectuum', 'short': {'ru': 'лат. текст, изд. 1913 г.', 'en': 'Latin text, 1913 printing'}},
                      'en': {'url': 'https://archive.org/details/chiefworks02spinuoft', 'short': {'ru': 'пер. Р. Элвса, 1898', 'en': 'tr. R. H. M. Elwes, 1898'}}},
        'places': {
            'I.D7': {'agent': '1d7', 'label': {'ru': 'Часть I, определение 7', 'en': 'Part I, Definition 7'}, 'links': {'la': 'https://la.wikisource.org/wiki/Ethica/Pars_prima_-_De_Deo'}},
            'III.P4': {'agent': '3p4', 'label': {'ru': 'Часть III, теорема 4', 'en': 'Part III, Proposition 4'}},
            'III.P6': {'agent': '3p6', 'label': {'ru': 'Часть III, теорема 6 и её доказательство', 'en': 'Part III, Proposition 6 and its proof'}},
            'III.P7': {'agent': '3p7', 'label': {'ru': 'Часть III, теорема 7 и её доказательство', 'en': 'Part III, Proposition 7 and its proof'}},
            'III.P8': {'agent': '3p8', 'label': {'ru': 'Часть III, теорема 8 и её доказательство', 'en': 'Part III, Proposition 8 and its proof'}}}},
    'monadologie': {
        'agent': 'leibniz', 'author': {'ru': 'Готфрид Вильгельм Лейбниц', 'en': 'Gottfried Wilhelm Leibniz'}, 'title': {'ru': 'Монадология', 'en': 'Monadology'}, 'original': 'fr',
        'witnesses': {'fr': {'url': 'https://archive.org/details/11172347bsb', 'short': {'ru': 'изд. К. И. Герхардта, 1885', 'en': 'ed. C. I. Gerhardt, 1885'}},
                      'en': {'url': 'https://archive.org/details/themonadology00leibuoft', 'short': {'ru': 'пер. Р. Латты, 1898', 'en': 'tr. Robert Latta, 1898'}},
                      'ru': {'url': 'https://commons.wikimedia.org/wiki/File:Izbrannyye_filosofskiye_sochineniya.pdf?page=373', 'short': {'ru': 'пер. Е. Н. Боброва, 1890', 'en': 'tr. E. N. Bobrov, 1890'},
                             'rights': {'basis': 'public-domain',
                                        'russia': 'Общественное достояние. Перевод издан в 1890 г.; охраняться сегодня он мог бы, только если бы переводчик умер после 1955 г. (после 1951 г., если он участвовал в Великой Отечественной войне или трудился в её годы) — ГК РФ ст. 1281, срок 70 лет со дня смерти, с продлением на четыре года для участников войны. В предисловии переводчик назван «Е. Н. Бобровъ»; исследователь Лейбница Е. А. Бобров умер в 1933 г. Редактор В. П. Преображенский умер в 1900 г.',
                                        'usa': 'Public domain: published 1890.',
                                        'eu': 'Public domain: a translator who published in 1890 would have to have died after 1955 for the translation to be protected (70 years p.m.a.); the Leibniz scholar E. A. Bobrov died in 1933.',
                                        'reasoning': 'Leibniz (d. 1716); translation published 1890 (Moscow). The preface credits article XV to «Е. Н. Бобровъ»; the Leibniz scholar of that name is Evgeny Aleksandrovich Bobrov (1867–1933), who wrote on the Monadology (1896) — the different middle initial is a misprint or a different person, which could not be settled. Either way a translator active in 1890 would have to have lived past 1955 for the translation to be protected now. Commons tags the file PD-old and PD-RusEmpire. Text transcribed from the page images in the pre-reform spelling of the print.'}}},
        'places': {k: {'agent': k, 'label': {'ru': f'§{k}', 'en': f'§{k}'}} for k in ('53', '54', '55')}},
    'gjentagelsen': {
        'agent': 'kierkegaard', 'author': {'ru': 'Сёрен Кьеркегор (Константин Констанций)', 'en': 'Søren Kierkegaard (Constantin Constantius)'}, 'title': {'ru': 'Повторение', 'en': 'Repetition'}, 'original': 'da',
        'witnesses': {'da': {'url': 'https://archive.org/details/srenkierkegaard10kiergoog', 'short': {'ru': 'Samlede Værker, т. III, 1901', 'en': 'Samlede Værker, vol. III, 1901'}}},
        'places': {'opening': {'agent': 'Part One, opening paragraph', 'label': {'ru': 'Первый абзац («Когда элеаты отрицали движение…»)', 'en': 'Opening paragraph (“When the Eleatics denied motion…”)'}}}},
    'heraclitus-book': {
        'agent': 'heraclitus', 'author': {'ru': 'Гераклит Эфесский', 'en': 'Heraclitus of Ephesus'}, 'title': {'ru': 'Фрагменты', 'en': 'Fragments'}, 'original': 'grc',
        'witnesses': {'grc': {'url': 'https://raw.githubusercontent.com/OpenGreekAndLatin/First1KGreek/master/data/tlg2018/tlg001/tlg2018.tlg001.1st1K-grc1.xml',
                              'sources': ['https://raw.githubusercontent.com/OpenGreekAndLatin/First1KGreek/master/data/tlg2018/tlg001/tlg2018.tlg001.1st1K-grc1.xml',
                                          'https://raw.githubusercontent.com/PerseusDL/canonical-greekLit/master/data/tlg0555/tlg004/tlg0555.tlg004.perseus-grc2.xml']},
                      'en': {'url': 'https://en.wikisource.org/wiki/Page:Early_Greek_philosophy_by_John_Burnet,_3rd_edition,_1920.djvu/148', 'short': {'ru': 'пер. Дж. Бёрнета, 1920', 'en': 'tr. John Burnet, 1920'}},
                      'ru': {'url': 'https://ru.wikisource.org/w/index.php?oldid=5738290', 'short': {'ru': 'пер. В. О. Нилендера, 1910', 'en': 'tr. V. O. Nilender, 1910'}}},
        'places': {
            'B12': {'agent': 'DK 22 B12', 'label': {'ru': 'DK 22 B12 (у Бёрнета — фр. 41–42)', 'en': 'DK 22 B12 (Burnet’s fr. 41–42)'},
                    'links': {'grc': 'https://archive.org/details/eusebiicaesarie03dindgoog', 'en': 'https://en.wikisource.org/wiki/Page:Early_Greek_philosophy_by_John_Burnet,_3rd_edition,_1920.djvu/150'}},
            'B30': {'agent': 'DK 22 B30', 'label': {'ru': 'DK 22 B30 (у Бёрнета — фр. 20)', 'en': 'DK 22 B30 (Burnet’s fr. 20)'},
                    'links': {'grc': 'https://archive.org/details/clemensalexandri02clem'}}}},
    'le-mythe-de-sisyphe': {
        'agent': 'camus', 'author': {'ru': 'Альбер Камю', 'en': 'Albert Camus'}, 'title': {'ru': 'Миф о Сизифе', 'en': 'The Myth of Sisyphus'}, 'original': 'fr',
        'witnesses': {'fr': {'url': 'https://fr.wikiquote.org/w/index.php?oldid=451559', 'opens': 'quotation', 'short': {'ru': 'по французскому Викицитатнику', 'en': 'as French Wikiquote gives it'},
                             'edition': 'Le Mythe de Sisyphe (1942), chapter « Le mythe de Sisyphe »; the quoted words as French Wikiquote gives them (revision 451559, citing Gallimard, coll. Folio essais, pp. 166 and 168).',
                             'notes': 'The full text could be checked in the UQAC edition (public domain in Canada): http://classiques.uqac.ca/classiques/camus_albert/mythe_de_sisyphe/mythe_de_sisyphe.html — it could not be reached. Internet Archive uploads of the French text are unauthorised and were not used. Only the sentences French Wikiquote carries for this chapter are given; its other paragraphs are cited by their locators.',
                             'reasoning_add': ' The quoted words are as French Wikiquote gives them; the same words are found in several independent quotations on the web (searched 30 October 2026); they could not be checked against a printed copy, and UQAC (where the text is public domain under Canadian law) was not reachable.'}},
        'places': {
            'return': {'label': chapter('возвращение к подножию', 'the return down')},
            'tragic': {'agent': 'Le mythe de Sisyphe, ¶ "Si ce mythe est tragique…"', 'label': chapter('конец абзаца о сознании героя', 'end of the paragraph on the hero’s consciousness')},
            'descent': {'label': chapter('спуск в скорби и в радости', 'the descent in sorrow and in joy')},
            'joy': {'label': chapter('безмолвная радость Сизифа', 'Sisyphus’s silent joy')},
            'subtle': {'label': chapter('миг, когда человек оглядывается на свою жизнь', 'the moment a man looks back on his life')},
            'ascent': {'label': chapter('камень скатывается с вершины', 'the stone rolls down from the summit')},
            'last': {'compose': ['Le mythe de Sisyphe, last ¶, opening', 'Le mythe de Sisyphe, last ¶, close'], 'label': chapter('последний абзац, начало и конец', 'last paragraph, opening and close')},
            'whole': {'label': {'ru': CHAPTER['ru'], 'en': CHAPTER['en']}}}},
    'nietzsche-et-la-philosophie': {
        'author': {'ru': 'Жиль Делёз', 'en': 'Gilles Deleuze'}, 'title': {'ru': 'Ницше и философия', 'en': 'Nietzsche and Philosophy'}, 'original': 'fr',
        'witnesses': {'fr': {'edition': 'Nietzsche et la philosophie. Paris: Presses Universitaires de France, 1962.', 'year': 1962, 'attribution': 'Gilles Deleuze, Nietzsche et la philosophie (Paris: PUF, 1962).', 'rights': {'basis': 'locator-only', **DELEUZE_RIGHTS}},
                      'en': {'edition': 'Nietzsche and Philosophy, tr. Hugh Tomlinson. New York: Columbia University Press, 1983.', 'year': 1983, 'translator': 'Hugh Tomlinson',
                             'attribution': 'Gilles Deleuze, Nietzsche and Philosophy, tr. Hugh Tomlinson (New York: Columbia University Press, 1983).', 'publisher_page': CUP_NP, 'rights': {'basis': 'locator-only', **DELEUZE_RIGHTS}}},
        'places': {'II.5': {'label': {'ru': 'Гл. II, конец §5', 'en': 'Ch. II, end of §5'}, 'url': CUP_NP}, 'II.6': {'label': {'ru': 'Гл. II, §6', 'en': 'Ch. II, §6'}, 'url': CUP_NP}}},
    'difference-et-repetition': {
        'author': {'ru': 'Жиль Делёз', 'en': 'Gilles Deleuze'}, 'title': {'ru': 'Различие и повторение', 'en': 'Difference and Repetition'}, 'original': 'fr',
        'witnesses': {'fr': {'edition': 'Différence et répétition. Paris: Presses Universitaires de France, 1968.', 'year': 1968, 'attribution': 'Gilles Deleuze, Différence et répétition (Paris: PUF, 1968).', 'rights': {'basis': 'locator-only', **DELEUZE_RIGHTS}},
                      'en': {'edition': 'Difference and Repetition, tr. Paul Patton. New York: Columbia University Press, 1994.', 'year': 1994, 'translator': 'Paul Patton',
                             'attribution': 'Gilles Deleuze, Difference and Repetition, tr. Paul Patton (New York: Columbia University Press, 1994).', 'publisher_page': CUP_DR, 'rights': {'basis': 'locator-only', **DELEUZE_RIGHTS}}},
        'places': {loc: {'label': label, 'url': CUP_DR} for loc, label in [
            ('intro', {'ru': 'Введение', 'en': 'Introduction'}),
            ('intro.generality', {'ru': 'Введение: повторение — не всеобщность', 'en': 'Introduction: repetition is not generality'}),
            ('intro.programme', {'ru': 'Введение: первое из четырёх положений', 'en': 'Introduction: the first of the four propositions'}),
            ('intro.corrections', {'ru': 'Введение: второе из четырёх положений — поправки Заратустры карлику и зверям', 'en': 'Introduction: the second of the four propositions — Zarathustra’s corrections of the dwarf and of the animals'}),
            ('intro.propositions', {'ru': 'Введение: четыре положения, общие Кьеркегору и Ницше', 'en': 'Introduction: the four propositions Kierkegaard and Nietzsche share'}),
            ('intro.insurmountable', {'ru': 'Введение: непреодолимое различие Кьеркегора и Ницше', 'en': 'Introduction: the insurmountable difference between Kierkegaard and Nietzsche'}),
            ('intro.movement', {'ru': 'Введение: метафизика, приведённая в движение', 'en': 'Introduction: metaphysics set in motion'}),
            ('intro.selective', {'ru': 'Введение: вечное возвращение как отбор', 'en': 'Introduction: the eternal return as selection'})]}},
}

# Reasons for a missing witness, stated without the tools' own messages.
REASONS = {('gjentagelsen', 'en'): "Walter Lowrie's translation (Princeton University Press, 1941) is on the Internet Archive only as an access-restricted lending copy; later translations (Hong and Hong, 1983, and others) are protected. No exact wording could be checked against an edition, so no English quotation is given."}
UNREACHABLE = [
    'deutschestextarchiv.de: the download paths answer scripts with a cookie challenge and robots.txt disallows them; not circumvented, so the 1887 issue of Die fröhliche Wissenschaft is used instead of the DTA text of the 1882 edition.',
    'classiques.uqac.ca (Le Mythe de Sisyphe, public domain in Canada): not reachable; the Wayback copy failed too.',
    'puf.com: not reachable, so no PUF page is given.',
    'sks.dk (Søren Kierkegaards Skrifter): not reachable.',
    'upload.wikimedia.org original files: rate-limited; the page renderings were read instead.']
def rights_of(r, add=''):
    """The agent's rights record with its jurisdictions named as such (not as languages)."""
    out = {'basis': r['basis']}
    for a, b in (('ru', 'russia'), ('us', 'usa'), ('eu', 'eu')):
        if r.get(a): out[b] = r[a]
    out['reasoning'] = ((r.get('reasoning') or '') + add).replace('(the djvu file could not be resolved in this session)', '(the scan could not be opened)').replace('given in source_refs', 'given in refs')
    return out
def year_of(v):
    return int(v) if isinstance(v, str) and re.fullmatch(r'\d{4}', v) else v

works, places, problems = {}, {}, []
for key, spec in WORKS.items():
    agent = src['works'].get(spec.get('agent')) if spec.get('agent') else None
    witnesses, absent = {}, []
    for lang, over in spec['witnesses'].items():
        a = (agent or {}).get('witnesses', {}).get(lang) or {}
        if a and not str(a.get('status', '')).startswith('obtained'): a = {}
        w = {}
        for field, name in (('edition', 'edition'), ('translator', 'translator'), ('year', 'year'), ('license', 'licence'), ('attribution', 'attribution'), ('notes', 'notes'), ('scan', 'scan')):
            if a.get(field) not in (None, 'None', ''): w[name] = year_of(a[field]) if field == 'year' else a[field]
        if a.get('rights'): w['rights'] = rights_of(a['rights'], over.get('reasoning_add', ''))
        for field in ('edition', 'translator', 'year', 'attribution', 'url', 'opens', 'sources', 'publisher_page', 'short', 'rights', 'notes'):
            if field in over: w[field] = over[field]
        witnesses[lang] = {k: w[k] for k in ('edition', 'translator', 'year', 'url', 'opens', 'sources', 'scan', 'publisher_page', 'licence', 'attribution', 'notes', 'rights', 'short') if k in w}
    for lang, a in ((agent or {}).get('witnesses') or {}).items():
        if lang not in witnesses and a and a.get('reason'): absent.append({'language': lang, 'reason': REASONS.get((key, lang), a['reason'])})
    if not agent and key == 'nietzsche-et-la-philosophie' or key == 'difference-et-repetition':
        absent.append({'language': 'ru', 'reason': 'A Russian translation exists (protected); not consulted.'})
    works[key] = {'author': spec['author'], 'title': spec['title'], 'original': spec['original'], 'witnesses': witnesses, **({'absent': absent} if absent else {})}
    for loc, p in spec['places'].items():
        if loc not in cited.get(key, []): problems.append(f'{key} {loc} is not cited by the stand'); continue
        texts, refs, notes = {}, [], []
        parts = [agent_places[(spec['agent'], x)] for x in p['compose']] if 'compose' in p else [agent_places[(spec['agent'], p['agent'])]] if 'agent' in p else []
        for lang in witnesses:
            values = [(part.get('texts') or {}).get(lang) for part in parts]
            if values and all(values): texts[lang] = ' […] '.join(v.strip() for v in values)
            elif any(values): problems.append(f'{key} {loc} {lang}: only some parts have text')
        for part in parts:
            for lang, ref in (part.get('source_refs') or {}).items():
                if ref: refs.append([lang, ref])
            if part.get('note') and part['note'] not in notes: notes.append(part['note'])
        entry = {'label': p['label'], 'texts': texts}
        if p.get('links'): entry['links'] = p['links']
        if p.get('url'): entry['url'] = p['url']
        if refs: entry['refs'] = refs
        if notes: entry['note'] = ' '.join(notes)
        places.setdefault(key, {})[loc] = entry
for key, locs in cited.items():
    for loc in locs:
        if loc not in places.get(key, {}): problems.append(f'cited place {key} {loc} has no entry')

PLACE_OF = {'spinoza.1d7': ('ethica', 'I.D7'), 'spinoza.3p6': ('ethica', 'III.P6'), 'spinoza.3p7': ('ethica', 'III.P7'), 'spinoza.3p8': ('ethica', 'III.P8')}
corrections = [{**{k: v for k, v in c.items() if k != 'place'}, 'place': ' '.join(PLACE_OF[c['place']])} for c in src['meta']['corrections'] if c['place'] in PLACE_OF]
module = {
    'schema': 'tos.bonsai.fragments.v1',
    'posture': {'ru': 'Точный текст мест других произведений, на которые опирается стенд, в оригинале и переводах; у каждого свидетеля — издание, источник, лицензия и основание показа. Охраняемое произведение показано короткой цитатой или только адресом места.',
                'en': 'The exact text of the places of other works the stand rests on, in the original and in translations; every witness with its edition, source, licence and basis for showing it. A protected work is shown by a short quotation or by the address of the place alone.'},
    'provenance': {'gathered': src['meta']['generated'], 'method': src['meta']['method'], 'corrections': corrections,
                   'unreachable': UNREACHABLE},
    'works': works, 'places': places}
json.dump(module, open(f'{REPO}/bonsai/zarathustra/fragments.json', 'w'), ensure_ascii=False, indent=1)
open(f'{REPO}/bonsai/zarathustra/fragments.json', 'a').write('\n')
print('works', len(works), 'places', sum(len(v) for v in places.values()), 'with text', sum(1 for v in places.values() for p in v.values() if p['texts']))
print('problems', problems)
