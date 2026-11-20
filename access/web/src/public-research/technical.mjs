// A technical section (TOS-D-0082): what is important but serves checking (rights, provenance, addresses, records)
// stands whole behind a door the reader opens on purpose, so that the reading keeps what is of use in view.
import {localeText,resolveLocale} from './locale.mjs';

const LABEL=['Технически','Technical details'];

/** A closed section holding `children`; it opens on a click or Enter and remembers nothing. */
export function technicalSection(doc,locale,children=[],{label=null}={}){
  const section=doc.createElement('details');section.className='tech-section';
  const summary=doc.createElement('summary');
  summary.textContent=label??(resolveLocale(locale())==='ru'?LABEL[0]:localeText(LABEL[1],locale()));
  section.append(summary,...children.filter(Boolean));
  return section;
}
