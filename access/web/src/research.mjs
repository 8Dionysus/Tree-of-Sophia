import '../constructor/style.css';
import {mountLiveResearch} from '../constructor/live-controller.mjs';

// Installable real-data entry. It does not import the constructor demo or its
// authored fixture catalog. The selected service owns discovery and reading.
const host=document.getElementById('tree');
void mountLiveResearch(host).catch(error=>{
  const message=document.createElement('p');message.className='loading';message.setAttribute('role','alert');
  message.textContent=error?.message??'Не удалось открыть исследование.';host.replaceChildren(message);
});
