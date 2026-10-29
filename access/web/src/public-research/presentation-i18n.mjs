import {PUBLIC_LOCALES,copyLocale,localeText} from './locale.mjs';
const EN_ACTIONS = {'lens.select':'Selected lens','edge.exclude':'Excluded link','edge.include':'Restored link','hypothesis.add':'Added personal thought','hypothesis.remove':'Removed personal thought','proposal.stage':'Added proposal','route.snapshot':'Saved route','route.remove':'Removed route','note.add':'Added note','note.update':'Updated note','note.remove':'Removed note','workspace.undo':'Undid change','workspace.redo':'Redid change'};
const RU_ACTIONS = {'lens.select':'Выбран взгляд','edge.exclude':'Связь исключена','edge.include':'Связь восстановлена','hypothesis.add':'Добавлена личная мысль','hypothesis.remove':'Личная мысль удалена','proposal.stage':'Добавлено предложение','route.snapshot':'Маршрут сохранён','route.remove':'Маршрут удалён','note.add':'Добавлена заметка','note.update':'Заметка изменена','note.remove':'Заметка удалена','workspace.undo':'Изменение отменено','workspace.redo':'Изменение возвращено'};
export const isEnglish = locale => copyLocale(locale)==='en';
export const kindLabel = (kind, locale) => ({interpretation:isEnglish(locale)?localeText('Interpretation',locale):'Интерпретация',relation:isEnglish(locale)?localeText('Relation',locale):'Связь'})[kind] || (isEnglish(locale)?localeText('Proposal',locale):'Предложение');
export const actionLabel = (action, locale) => localeText((isEnglish(locale)?EN_ACTIONS:RU_ACTIONS)[action]??'',locale) || (isEnglish(locale)?localeText('Workspace change',locale):'Изменение исследования');
export const researchQuestionLine = (value, locale) => {
  const question = typeof value === 'string' ? value.trim() : '';
  if (question) return `${isEnglish(locale) ? localeText('Research question',locale) : 'Вопрос исследования'}: ${question}`;
  return isEnglish(locale) ? localeText('No research question set.',locale) : 'Вопрос исследования не задан.';
};
export function proposalStatement(value, locale) {
  if (typeof value !== 'string') return '';
  for (const [prefix,en,ru] of [['Objection to thesis: ','Objection to thesis: ','Возражение к тезису: '],['Revises earlier thesis: ','Revises earlier thesis: ','Пересмотр прежнего тезиса: '],['Objection: ','Objection: ','Возражение: ']]) {
    if (value.startsWith(prefix)) return (isEnglish(locale)?localeText(en,locale):ru)+value.slice(prefix.length);
  }
  return value;
}
// Only the known generated relation IDs own these prefixes.  A user's
// arbitrary proposal that happens to begin with the same words stays literal.
export function proposalDisplay(proposal, locale) {
  const value=typeof proposal?.statement==='string'?proposal.statement:'';
  if(proposal?.kind!=='relation')return value;
  if(proposal.id?.startsWith('objection-link-')&&value.startsWith('Objection to thesis: '))return proposalStatement(value,locale);
  if(proposal.id?.startsWith('revision-link-')&&value.startsWith('Revises earlier thesis: '))return proposalStatement(value,locale);
  return value;
}
export function hypothesisTitle(hypothesis, locale) {
  const value=typeof hypothesis?.title==='string'?hypothesis.title:'';
  const options=hypothesis?.id?.startsWith('objection-')?['Objection: ','Возражение: ']:
    hypothesis?.id?.startsWith('revision-')?['Reply or revised thesis: ','Ответ или пересмотренный тезис: ']:null;
  if(!options)return value;
  const base=options[0].slice(0,-2);
  // Earlier generated titles used the active UI label. Recognize those exact
  // prefixes for generated IDs only; the user's body stays byte-for-byte.
  const prefixes=[...new Set([...options,...PUBLIC_LOCALES.flatMap(language=>[localeText(options[0],language),`${localeText(base,language)}: `])])].sort((a,b)=>b.length-a.length);
  const prefix=prefixes.find(item=>value.startsWith(item));
  return prefix?(isEnglish(locale)?localeText(options[0],locale):options[1])+value.slice(prefix.length):value;
}
// Service errors can contain schema fields and implementation details. The UI
// presents bounded categories while the validation and stored packet stay intact.
export function displayPublicError(error, locale) {
  const raw=String(error?.message ?? error ?? ''); const en=isEnglish(locale);
  if (error?.code === 'agent-question-changed') return en?localeText('The question changed after this task was exported. Keep the response for review and export a new task before applying it.',locale):'После экспорта задания вопрос изменился. Сохраните ответ для просмотра и экспортируйте новое задание перед применением.';
  if (/two exact grounds|два точных основания/i.test(raw)) return en?localeText('A thesis needs at least two exact grounds.',locale):'Для тезиса нужны два точных основания.';
  if (/exact source for the objection/i.test(raw)) return en?localeText('Choose an exact source for the objection.',locale):'Выберите точный источник для возражения.';
  if (/json/i.test(raw)) return en?localeText('The file or response is not valid JSON.',locale):'Файл или ответ содержит некорректный JSON.';
  if (/64 KB/i.test(raw)) return en?localeText('The task or response exceeds 64 KB.',locale):'Задание или ответ превышает 64 КБ.';
  if (/12 MB/i.test(raw)) return en?localeText('The research file exceeds 12 MB.',locale):'Файл исследования превышает 12 МБ.';
  if (/empty|non-empty/i.test(raw)) return en?localeText('Fill in the required fields.',locale):'Заполните обязательные поля.';
  if (/at most|character|exceeds/i.test(raw)) return en?localeText('A field exceeds the allowed length.',locale):'Поле превышает допустимую длину.';
  if (/source|reference|stale/i.test(raw)) return en?localeText('Check the exact sources and try again.',locale):'Проверьте точные источники и повторите попытку.';
  if (/conflict|already|revision|changed/i.test(raw)) return en?localeText('The research has changed or conflicts with existing data. Review it before trying again.',locale):'Исследование изменилось или конфликтует с существующими данными. Проверьте его перед повторной попыткой.';
  return en?localeText('Could not complete this action. Check the input and try again.',locale):'Не удалось выполнить действие. Проверьте данные и повторите попытку.';
}

// Import conflicts must keep the exact record/address that needs attention.
export function workspaceConflictLabel(conflict,locale){
  if(typeof conflict!=='string')return '';
  const ru=copyLocale(locale)==='ru';
  const prefix='workspace source address ',suffix=' cannot be imported because its shelf owner packet is unavailable or conflicted.';
  if(conflict.startsWith(prefix)&&conflict.endsWith(suffix)){
    const id=conflict.slice(prefix.length,-suffix.length);
    return ru?`Адрес источника ${id} нельзя импортировать: данные полки отсутствуют или конфликтуют.`:localeText('The source address {0} cannot be imported: shelf data is missing or conflicts.',locale,[id]);
  }
  if(conflict.startsWith('The current research workspace'))return ru?'Текущее исследование не пусто и отличается от импортируемого.':localeText('The current research workspace is not empty and differs from the incoming data.',locale);
  if(conflict.startsWith('The workspace changed'))return ru?'Исследование изменилось в другой вкладке; заново откройте и проверьте файл.':localeText('The research changed in another tab. Open and review the file again.',locale);
  const [key,...rest]=conflict.split(': '),names=ru?{notes:'Заметка',readings:'Положение чтения',preferences:'Настройка','shelf record':'Запись полки','shelf collection':'Коллекция полки'}:{notes:'Note',readings:'Reading position',preferences:'Preference','shelf record':'Shelf record','shelf collection':'Shelf collection'};
  if(!Object.hasOwn(names,key))return conflict;
  return `${localeText(names[key],locale)}: ${rest.join(': ')}`;
}
