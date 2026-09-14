// Explain query execution only. Source wording comes from the selected reader;
// unknown reason kinds retain a visible gap and their separate exact packet.
export function inclusionSummary(reason,{language='ru',nodeLabel=()=>null,relationLabel=()=>null,predicateLabel=id=>id}={}){
  const ru=language==='ru',lines=[];
  const known={
    origin:ru?'Материал выбран началом этого раскрытия.':'This material was selected as the origin of this expansion.',
    focus:ru?'Материал выбран началом этого раскрытия.':'This material was selected as the origin of this expansion.',
    'origin-endpoint':ru?'Этот предмет — участник выбранной исходной связи.':'This object is an endpoint of the selected origin relation.',
    'context-endpoint':ru?'Этот предмет добавлен как участник возвращённой связи.':'This object was included as an endpoint of a returned relation.',
    traversal:ru?'Материал найден при переходе по связям.':'This material was found by following relations.',
    'identity-carrier':ru?'Это другой носитель той же объявленной идентичности, а не новое смысловое соседство.':'This is another carrier of the same declared identity, not a new semantic neighbor.',
  };
  lines.push(Object.hasOwn(known,reason?.kind)?known[reason.kind]:ru?'Для этой причины включения человеческое описание пока не поддерживается.':'A human description of this inclusion reason is not yet supported.');
  if(reason?.kind==='traversal'||reason?.kind==='identity-carrier'){
    const via=typeof reason.via_node_id==='string'?nodeLabel(reason.via_node_id):null;
    if(via)lines.push(ru?`Переход от: «${via}».`:`Reached from: “${via}”.`);
  }
  if(reason?.kind==='traversal'){
    const via=typeof reason.via_relation_id==='string'?relationLabel(reason.via_relation_id):null;
    if(via)lines.push(ru?`Через связь: «${via}».`:`Via relation: “${via}”.`);
    if(Number.isSafeInteger(reason.depth)&&reason.depth>=0)lines.push(ru?`Шагов от начала: ${reason.depth}.`:`Hops from the origin: ${reason.depth}.`);
  }
  const directions=ru?{incoming:'входящие',outgoing:'исходящие',either:'в обе стороны'}:{incoming:'incoming',outgoing:'outgoing',either:'both directions'};
  if(Object.hasOwn(directions,reason?.query?.direction))lines.push((ru?'Направление запроса: ':'Query direction: ')+directions[reason.query.direction]+'.');
  if(Number.isSafeInteger(reason?.query?.max_depth)&&reason.query.max_depth>=0)lines.push((ru?'Предельная глубина запроса: ':'Query depth limit: ')+reason.query.max_depth+'.');
  if(Array.isArray(reason?.query?.predicate_ids)&&reason.query.predicate_ids.length&&reason.query.predicate_ids.every(id=>typeof id==='string'))
    lines.push((ru?'Фильтр связей: ':'Relation filter: ')+reason.query.predicate_ids.map(id=>predicateLabel(id)||id).join(', ')+'.');
  return lines;
}
