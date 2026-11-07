#!/usr/bin/env python3
"""Index a Wiktextract (kaikki.org) language extract as a local SQLite lexicon.

The philological tools of a bonsai read Wiktionary through this index: which
written forms a lemma takes, which spellings Wiktionary marks as obsolete or
archaic spellings of another word, and each entry's etymology. The extract and
the index stay outside the checkout; a bonsai module keeps only what it uses,
with the entry it came from.

    python3 lexicon.py <kaikki-Language.jsonl> <out.sqlite> --lang de

The extract is the English Wiktionary's section for one language, published
by kaikki.org (Tatu Ylonen, Wiktextract, LREC 2022). Wiktionary's text is
available under CC BY-SA 4.0; a record built from it keeps its entry title and
link so that it can be attributed.
"""
import argparse, hashlib, json, os, re, sqlite3, sys, unicodedata

# Spellings Wiktionary files as variants of another written form; Swiss and Liechtenstein
# German keeps ss where the standard writes ß.
SPELLING_TAGS = {'obsolete', 'archaic', 'dated', 'nonstandard', 'misspelling', 'pre-reform', 'Switzerland'}
# A form-of sense names an inflected form only when it carries an inflectional
# category; agent nouns, diminutives, nominalisations and aspect partners are
# words of their own.
INFLECTION_TAGS = {'nominative', 'genitive', 'dative', 'accusative', 'vocative', 'instrumental', 'locative',
                   'prepositional', 'singular', 'plural', 'first-person', 'second-person', 'third-person', 'past',
                   'present', 'preterite', 'participle', 'imperative', 'subjunctive', 'subjunctive-i', 'subjunctive-ii',
                   'infinitive', 'comparative', 'superlative', 'gerund', 'short-form', 'future'}
DERIVATION_TAGS = {'agent', 'diminutive', 'augmentative', 'nominalization', 'noun-from-verb', 'abbreviation',
                   'acronym', 'initialism', 'clipping', 'perfective', 'imperfective'}
# A gloss that files the entry as an earlier spelling ("Former spelling of Maß which is no longer standard").
OLD_GLOSS = re.compile(r'^(Obsolete|Archaic|Former|Dated|Older|Superseded|Deprecated)\s+(?:form|spelling)\s+of\s+([^\s,;.()]+)', re.I)
SKIP_FORM_TAGS = {'table-tags', 'inflection-template', 'class', 'romanization', 'auxiliary'}


def fold(word, lang):
    """The key a form is looked up by: NFC, no stress marks; Russian ё read as е."""
    text = unicodedata.normalize('NFD', word)
    text = ''.join(ch for ch in text if ch not in '́̀')
    text = unicodedata.normalize('NFC', text)
    if lang == 'ru':
        text = text.replace('ё', 'е').replace('Ё', 'Е')
    return text


def target_of(sense, field):
    """The word a form-of or alt-of sense points to (Wiktionary's own link first)."""
    items = sense.get(field) or []
    if not items:
        return None
    word = items[0].get('word', '')
    links = sense.get('links') or []
    for text, target in links:
        if word.startswith(text):
            return text
    target = re.split(r'\s+(?:which|that|\()', word, maxsplit=1)[0].strip()
    # A description in place of a word ("the third person singular") names no form.
    return target if target and len(target.split()) <= 2 else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('extract')
    ap.add_argument('out')
    ap.add_argument('--lang', required=True)
    args = ap.parse_args()
    if os.path.exists(args.out):
        os.remove(args.out)
    db = sqlite3.connect(args.out)
    db.executescript('''
      create table meta(key text primary key, value text);
      create table entry(id text primary key, word text, key text, pos text, etymology text,
                         etymology_templates text, glosses text, spelling_of text, spelling_tags text,
                         form_of text, form_tags text);
      create table form(key text, form text, lemma text, pos text, tags text, entry text);
      create index entry_key on entry(key);
      create index form_key on form(key);
    ''')
    digest = hashlib.sha256()
    entries = forms = 0
    with open(args.extract, 'rb') as fh:
        for raw in fh:
            digest.update(raw)
            d = json.loads(raw)
            if d.get('lang_code') != args.lang:
                continue
            word, pos = d.get('word'), d.get('pos')
            if not word or not pos:
                continue
            senses = d.get('senses') or []
            spelling_of = spelling_tags = form_of = form_tags = None
            # An entry is a spelling of another word only when every sense of it is (Unruhe, besides an archaic
            # spelling of Unruh, is the word for unrest).
            variant_senses = [s for s in senses if ('alt-of' in set(s.get('tags') or []) and set(s.get('tags') or []) & SPELLING_TAGS)
                              or OLD_GLOSS.match((s.get('glosses') or [''])[0])]
            if senses and len(variant_senses) == len(senses):
                first = variant_senses[0]
                tags = set(first.get('tags') or [])
                if first.get('alt_of'):
                    spelling_of, spelling_tags = target_of(first, 'alt_of'), sorted(tags & SPELLING_TAGS)
                else:
                    m = OLD_GLOSS.match(first['glosses'][0])
                    spelling_of, spelling_tags = m.group(2), [m.group(1).lower()]
            for s in senses:
                tags = set(s.get('tags') or [])
                if 'form-of' in tags and not form_of and tags & INFLECTION_TAGS and not tags & DERIVATION_TAGS:
                    target = target_of(s, 'form_of')
                    if target and len(target) > 1:
                        form_of, form_tags = target, sorted(tags - {'form-of'})
            eid = senses[0].get('id') if senses and senses[0].get('id') else f'{word}:{pos}:{entries}'
            glosses = [g for s in senses for g in (s.get('glosses') or [])][:6]
            db.execute('insert or ignore into entry values (?,?,?,?,?,?,?,?,?,?,?)', (
                eid, word, fold(word, args.lang), pos, d.get('etymology_text'),
                json.dumps(d.get('etymology_templates') or [], ensure_ascii=False),
                json.dumps(glosses, ensure_ascii=False), spelling_of,
                json.dumps(spelling_tags) if spelling_tags else None,
                form_of, json.dumps(form_tags) if form_tags else None))
            entries += 1
            lemma = form_of or word
            db.execute('insert into form values (?,?,?,?,?,?)', (fold(word, args.lang), word, lemma, pos,
                                                                  json.dumps(form_tags or []), eid))
            if form_of:
                continue
            seen = set()
            for f in d.get('forms') or []:
                text, tags = f.get('form'), f.get('tags') or []
                # Analytic forms ("habe gesagt", "sage ab") are phrases, not written forms of one word.
                if (not text or set(tags) & (SKIP_FORM_TAGS | DERIVATION_TAGS) or text in ('-', '—')
                        or ' ' in text.strip()):
                    continue
                key = (text, tuple(sorted(tags)))
                if key in seen:
                    continue
                seen.add(key)
                db.execute('insert into form values (?,?,?,?,?,?)', (fold(text, args.lang), text, word, pos,
                                                                      json.dumps(sorted(tags)), eid))
                forms += 1
    db.executemany('insert into meta values (?,?)', [
        ('lang', args.lang), ('extract', os.path.basename(args.extract)), ('extract_sha256', digest.hexdigest()),
        ('entries', str(entries)), ('forms', str(forms)),
        ('source', 'English Wiktionary via kaikki.org (Wiktextract); CC BY-SA 4.0')])
    db.commit()
    print(json.dumps({'lang': args.lang, 'entries': entries, 'forms': forms, 'extract_sha256': digest.hexdigest()}))


if __name__ == '__main__':
    sys.exit(main())
