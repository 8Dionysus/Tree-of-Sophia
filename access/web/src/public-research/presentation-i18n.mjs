const EN_ACTIONS = {'lens.select':'Selected lens','edge.exclude':'Excluded link','edge.include':'Restored link','hypothesis.add':'Added personal thought','hypothesis.remove':'Removed personal thought','proposal.stage':'Added proposal','route.snapshot':'Saved route','route.remove':'Removed route','note.add':'Added note','note.update':'Updated note','note.remove':'Removed note','workspace.undo':'Undid change','workspace.redo':'Redid change'};
const RU_ACTIONS = {'lens.select':'Выбран взгляд','edge.exclude':'Связь исключена','edge.include':'Связь восстановлена','hypothesis.add':'Добавлена личная мысль','hypothesis.remove':'Личная мысль удалена','proposal.stage':'Добавлено предложение','route.snapshot':'Маршрут сохранён','route.remove':'Маршрут удалён','note.add':'Добавлена заметка','note.update':'Заметка изменена','note.remove':'Заметка удалена','workspace.undo':'Изменение отменено','workspace.redo':'Изменение возвращено'};
export const isEnglish = locale => String(typeof locale === 'function' ? locale() : locale || 'ru').toLowerCase().startsWith('en');
export const kindLabel = (kind, locale) => ({interpretation:isEnglish(locale)?'Interpretation':'Интерпретация',relation:isEnglish(locale)?'Relation':'Связь'})[kind] || (isEnglish(locale)?'Proposal':'Предложение');
export const actionLabel = (action, locale) => (isEnglish(locale)?EN_ACTIONS:RU_ACTIONS)[action] || (isEnglish(locale)?'Workspace change':'Изменение исследования');
export const researchQuestionLine = (value, locale) => {
  const question = typeof value === 'string' ? value.trim() : '';
  if (question) return `${isEnglish(locale) ? 'Research question' : 'Вопрос исследования'}: ${question}`;
  return isEnglish(locale) ? 'No research question set.' : 'Вопрос исследования не задан.';
};
export function proposalStatement(value, locale) {
  if (typeof value !== 'string') return '';
  for (const [prefix,en,ru] of [['Objection to thesis: ','Objection to thesis: ','Возражение к тезису: '],['Revises earlier thesis: ','Revises earlier thesis: ','Пересмотр прежнего тезиса: '],['Objection: ','Objection: ','Возражение: ']]) {
    if (value.startsWith(prefix)) return (isEnglish(locale)?en:ru)+value.slice(prefix.length);
  }
  return value;
}
// Service errors can contain schema fields and implementation details. The UI
// presents bounded categories while the validation and stored packet stay intact.
export function displayPublicError(error, locale) {
  const raw=String(error?.message ?? error ?? ''); const en=isEnglish(locale);
  if (/two exact grounds|два точных основания/i.test(raw)) return en?'A thesis needs at least two exact grounds.':'Для тезиса нужны два точных основания.';
  if (/exact source for the objection/i.test(raw)) return en?'Choose an exact source for the objection.':'Выберите точный источник для возражения.';
  if (/json/i.test(raw)) return en?'The file or response is not valid JSON.':'Файл или ответ содержит некорректный JSON.';
  if (/64 KB/i.test(raw)) return en?'The task or response exceeds 64 KB.':'Задание или ответ превышает 64 КБ.';
  if (/12 MB/i.test(raw)) return en?'The research file exceeds 12 MB.':'Файл исследования превышает 12 МБ.';
  if (/empty|non-empty/i.test(raw)) return en?'Fill in the required fields.':'Заполните обязательные поля.';
  if (/at most|character|exceeds/i.test(raw)) return en?'A field exceeds the allowed length.':'Поле превышает допустимую длину.';
  if (/source|reference|stale/i.test(raw)) return en?'Check the exact sources and try again.':'Проверьте точные источники и повторите попытку.';
  if (/conflict|already|revision|changed/i.test(raw)) return en?'The research has changed or conflicts with existing data. Review it before trying again.':'Исследование изменилось или конфликтует с существующими данными. Проверьте его перед повторной попыткой.';
  return en?'Could not complete this action. Check the input and try again.':'Не удалось выполнить действие. Проверьте данные и повторите попытку.';
}
