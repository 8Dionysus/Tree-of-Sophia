import {t} from './ui-i18n.mjs';

let normalize;
const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', {fatal:true});
const packet=value=>JSON.stringify(value,(_key,item)=>typeof item==='number'&&!Number.isFinite(item)?'__nonfinite_json_number__':item);

export function installInterfaceRules(runtime) {
  if (typeof runtime?.normalize_interface_preferences_wasm_v1 !== 'function')
    throw new TypeError('Interface preferences WASM rule is unavailable');
  normalize = runtime.normalize_interface_preferences_wasm_v1;
}

export function normalizeInterfacePreferences(value) {
  if (!normalize) return null;
  try {
    const result=JSON.parse(decoder.decode(normalize(encoder.encode(packet(value)))));
    // JSON.stringify erases the sign of zero; the legacy reader preserves it.
    for(const id of Object.keys(result.positions))for(const axis of ['x','y'])
      if(Object.is(value.positions?.[id]?.[axis],-0))result.positions[id][axis]=-0;
    return result;
  } catch (error) {
    const code = String(error);
    if (code.includes('invalid_interface_size')) throw new Error(t('Сохранённый размер окна повреждён.'), {cause:error});
    if (code.includes('invalid_interface_position')) throw new Error(t('Сохранённое положение окна повреждено.'), {cause:error});
    if (code.includes('invalid_interface_control')) throw new Error(t('Настройки управления или оформления повреждены.'), {cause:error});
    throw new Error(t('Настройки интерфейса не удалось прочитать.'), {cause:error});
  }
}
