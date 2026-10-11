import {t} from './ui-i18n.mjs';
import {normalizePose} from './view-state-rust.mjs';

export function validatePose(value){
  const normalized=normalizePose(value);
  if(normalized===null)throw new Error(t("Не удалось прочитать положение сохранённого места."));
  return normalized;
}
