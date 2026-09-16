import {afterEach,test} from 'vitest';
import assert from 'node:assert/strict';
import {evidenceConclusionState} from './evidence-panel.mjs';
import {setUiLanguage} from './ui-i18n.mjs';

afterEach(()=>setUiLanguage('ru'));

test.each([
  [false,'Сведения','Вывод пока не установлен.','Information','No conclusion has been established yet.','Información','Aún no se ha establecido ninguna conclusión.'],
  [undefined,'Сведения','Вывод пока не установлен.','Information','No conclusion has been established yet.','Información','Aún no se ha establecido ninguna conclusión.'],
  [true,'Выводы',null,'Findings',null,'Conclusiones',null],
])('evidence conclusion state keeps the posture explicit: %s',(canConclude,ruHeading,ruState,enHeading,enState,esHeading,esState)=>{
  const conclusion=canConclude===undefined?{}:{can_conclude:canConclude};
  setUiLanguage('ru');let state=evidenceConclusionState(conclusion);assert.equal(state.canConclude,canConclude===true);assert.equal(String(state.heading),ruHeading);assert.equal(state.state===null?null:String(state.state),ruState);
  setUiLanguage('en');state=evidenceConclusionState(conclusion);assert.equal(String(state.heading),enHeading);assert.equal(state.state===null?null:String(state.state),enState);
  setUiLanguage('es');state=evidenceConclusionState(conclusion);assert.equal(String(state.heading),esHeading);assert.equal(state.state===null?null:String(state.state),esState);
});
