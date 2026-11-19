// The public site's build carries the site's copy of the layers under layers/sha256/<hex> of its output: the
// stands' word layers, the public corpus and the stand copies of media files, from every home of the checkout
// (bonsai/lib/layers.mjs). Each site build writes them after its pages, so a page never addresses a
// layer its site lacks.
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {placeLayers} from '../../bonsai/lib/layers.mjs';

/** @returns {import('vite').Plugin} */
export function siteLayers({repo=fileURLToPath(new URL('../..',import.meta.url))}={}){
  let outDir='';
  return {
    name:'tos-site-layers',
    apply:'build',
    configResolved(config){outDir=resolve(config.root,config.build.outDir);},
    async closeBundle(){
      const {homes,files}=await placeLayers(repo,join(outDir,'layers'),{log:line=>console.log(line)});
      console.log(`layers: ${files} files of ${homes.join(', ')}`);
    },
  };
}
