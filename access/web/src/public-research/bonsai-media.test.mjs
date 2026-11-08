import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {describe,expect,it} from 'vitest';
import {addMedia,mediaFromObservation,mediaIssues,readMedia} from '../../../../bonsai/lib/media.mjs';
import {MEDIA_HOSTS} from '../../../../bonsai/lib/media-hosts.mjs';
import {captionLines,captionLinks,fittedImage,fittedSize} from './media-view.mjs';

const read=path=>JSON.parse(readFileSync(new URL(`../../../../bonsai/antiquity/${path}`,import.meta.url),'utf8'));
const digest=text=>createHash('sha256').update(text,'utf8').digest('hex');
const PDM='https://creativecommons.org/publicdomain/mark/1.0/';

// A provider observation as the Tree's media bridge writes it (media.observe), cut to what the module reads.
const observation=(overrides={})=>({
  schema_version:'tos_media_provider_observation_v1',provider:'rijksmuseum',requested_object:'RP-P-2007-362',observed_at:'2026-11-08T05:12:55.850659369Z',
  requests:[{role:'object-record',sha256:'a'.repeat(64)}],
  object:{
    record_url:'https://id.rijksmuseum.nl/200488935',repository:'Rijksmuseum, Amsterdam',
    identifiers:[{scheme:'Rijksmuseum object number',value:'RP-P-2007-362'},{scheme:'Handle',value:'http://hdl.handle.net/10934/RM0001.COLLECT.450243'}],
    titles:[
      {text:'Portret van Marcus Tullius Cicero',language:'nl',role:'http://vocab.getty.edu/aat/300404670'},
      {text:'Illustrium philosophorum et poetarum effigies',language:'nl',role:'series title'},
      {text:'Cicero',language:'nl',role:'title on object'},
    ],
    production_statements:[
      {text:'René Boyvin (mentioned on object)',language:'en'},{text:'René Boyvin (vermeld op object)',language:'nl'},
      {text:'printmaker: René Boyvin (mentioned on object)',language:'en'},
    ],
    dating:{statement:'1566',begin_year:1566,end_year:1566},classifications:['print'],materials:['paper'],
    dimensions:[{kind:'height',value:'175',unit_uri:'http://vocab.getty.edu/aat/300379097'},{kind:'width',value:'121',unit_uri:'http://vocab.getty.edu/aat/300379097'}],
    credit_lines:[{text:'Aankoop uit het F.G. Waller-Fonds',language:'nl'},{text:'Purchased with the support of the F.G. Waller-Fonds',language:'en'}],
    depicted:[{authority_uris:['http://www.wikidata.org/entity/Q1541']}],
  },
  rights:[{scope:'image',provider_field:'subject_to',value:PDM,statement_uri:PDM,label:'Public Domain',stated_in:'https://data.rijksmuseum.nl/202488935?_profile=la-framed'}],
  images:[{role:'primary',form:'image_service',image_service:{api_version:'3',service_id:'https://iiif.micr.io/qnEHd',width_pixels:2748,height_pixels:4044,info_fingerprint:{sha256:'b'.repeat(64),checked_at:'2026-11-08T05:12:58.0Z'}}}],
  ...overrides,
});
const added=(extra={})=>addMedia({module:null,identities:{schema:'tos.bonsai.identities.v1'},observation:observation(),subject:'cicero',role:'author-portrait',artifact:'european-printmaking/rijksmuseum-amsterdam/rp-p-2007-362',digest,bonsai:'antiquity',...extra});

describe('a media module',()=>{
  it('turns a provider observation into a work, its image and a use, with their Tree identities',()=>{
    const {module,identities,workId,imageId}=added();
    expect(workId).toBe('rijksmuseum:RP-P-2007-362');expect(imageId).toBe('rijksmuseum:RP-P-2007-362/whole');
    const work=module.works[workId];
    expect(work.titles.map(title=>title.role)).toEqual(['title','series','inscribed']);
    // Of the English statements, the one another contains goes: the maker is named once, in the provider's fullest words.
    expect(work.made).toEqual([{text:'printmaker: René Boyvin (mentioned on object)',language:'en'}]);
    expect(work.dating).toMatchObject({kind:'date-assertion',value:'1566',certainty:'exact',source_wording:{text:'1566',language:null}});
    expect(work.size_mm).toEqual([175,121]);
    expect(work.depicts).toEqual(['http://www.wikidata.org/entity/Q1541']);
    expect(module.images[imageId]).toMatchObject({service:'https://iiif.micr.io/qnEHd',width:2748,height:4044,rights:{statement:PDM}});
    expect(module.uses).toEqual([{subject:'cicero',image:imageId,role:'author-portrait'}]);
    expect(identities.artifacts[workId]).toEqual({tree:'tos.artifact.european-printmaking.rijksmuseum-amsterdam.rp-p-2007-362',path:'european-printmaking/rijksmuseum-amsterdam/rp-p-2007-362',status:'to-mint'});
    // The representation is named after its image service, as the Tree's media planting names it.
    expect(identities.representations[imageId].tree).toBe(`tos.image-service.sha256.${digest('https://iiif.micr.io/qnEHd')}`);
    expect(mediaIssues(module,{subjects:new Set(['cicero']),identities,digest})).toEqual([]);
  });

  it('keeps a dating of several timespans as their interval in the provider\'s words, and keeps no dating where the provider gives none',()=>{
    const several=mediaFromObservation(observation({object:{...observation().object,dating:{statement:'1606; 1613',begin_year:1606,end_year:1613}}}));
    expect(several.work.dating).toMatchObject({kind:'interval-assertion',interval:{start:'1606',end:'1613'},source_wording:{text:'1606; 1613'}});
    const about=mediaFromObservation(observation({object:{...observation().object,dating:{statement:'ca. 1825–30',begin_year:1825,end_year:1830}}}));
    expect(about.work.dating.certainty).toBe('approximate');
    const none=mediaFromObservation(observation({object:{...observation().object,dating:{statement:null,begin_year:1566,end_year:1566}}}));
    expect(none.work.dating).toBeNull();
  });

  it('refuses an image without an image service or whose rights do not let a stand show it',()=>{
    expect(()=>mediaFromObservation(observation({images:[{role:'primary',form:'file',file_url:'https://images.example/x.jpg',image_service:null}]}))).toThrow(/no image service/);
    expect(()=>mediaFromObservation(observation({rights:[{scope:'image',provider_field:'subject_to',value:'https://rightsstatements.org/vocab/InC/1.0/',statement_uri:'https://rightsstatements.org/vocab/InC/1.0/',label:'In Copyright',stated_in:'https://data.rijksmuseum.nl/1'}]}))).toThrow(/rights/);
    expect(()=>added({role:'decoration'})).toThrow(/role/);
    expect(()=>added({artifact:'rijksmuseum/rp-p-2007-362'})).toThrow(/tradition/);
  });

  it('names what breaks: an unknown subject, a host outside the list, a graft map that names another representation, an image no use shows',()=>{
    const {module,identities,imageId}=added();
    const broken=structuredClone(module);
    broken.uses.push({subject:'nobody',image:imageId,role:'comparison'});
    broken.images[imageId].service='https://images.example.org/x';
    broken.images['rijksmuseum:RP-P-2007-362/detail']={...module.images[imageId]};
    const issues=mediaIssues(broken,{subjects:new Set(['cicero']),identities,digest});
    expect(issues).toContain('use 1: nobody is not a subject of the stand');
    expect(issues).toContain(`image ${imageId}: images.example.org is not a media host; add it to bonsai/lib/media-hosts.mjs`);
    expect(issues).toContain(`image ${imageId}: the graft map's representation is not named after its service`);
    expect(issues).toContain('image rijksmuseum:RP-P-2007-362/detail: no use shows it');
    expect(issues).toContain('image rijksmuseum:RP-P-2007-362/detail: no Tree representation in the graft map');
  });

  it('gives a subject its media in the order of their roles',()=>{
    const first=added(),second=addMedia({module:first.module,identities:first.identities,observation:observation(),subject:'cicero',role:'comparison',artifact:'european-printmaking/rijksmuseum-amsterdam/rp-p-2007-362',digest,view:'whole'});
    second.module.uses.reverse();
    expect(readMedia(second.module).forSubject('cicero').map(item=>item.role)).toEqual(['author-portrait','comparison']);
    expect(readMedia(second.module).forSubject('plato')).toEqual([]);
  });
});

describe('the Antiquity media',()=>{
  const media=read('media.json'),identities=read('identities.json'),collection=read('collection.json');
  const subjects=new Set(collection.entities.map(entry=>entry.id));
  it('holds its contract, its subjects and its graft map',()=>{
    expect(mediaIssues(media,{subjects,identities,digest})).toEqual([]);
  });
  it('shows a portrait for each of the seven authors it binds, from public-domain image services on the listed hosts',()=>{
    const portraits=media.uses.filter(use=>use.role==='author-portrait').map(use=>use.subject).sort();
    expect(portraits).toEqual(['aristotle','cicero','diogenes','hippocrates','plato','seneca','socrates']);
    for(const image of Object.values(media.images)){
      expect(MEDIA_HOSTS).toContain(new URL(image.service).host);
      expect(image.rights.statement).toBe(PDM);
    }
  });
});

describe('a medium on a card',()=>{
  const {module}=added(),[item]=readMedia(module).forSubject('cicero');
  const t=key=>({'media_author-portrait':'Портрет автора',mediaInscribed:'Надпись на предмете',mediaSeries:'Серия',mediaSize:'{0} × {1} мм'})[key]??key;
  it('asks the image service for a small whole copy in the image\'s own proportions',()=>{
    expect(fittedImage('https://iiif.micr.io/qnEHd',480)).toBe('https://iiif.micr.io/qnEHd/full/!480,480/0/default.jpg');
    expect(fittedSize({width:2748,height:4044})).toEqual({width:326,height:480});
    expect(fittedSize({width:300,height:200})).toEqual({width:300,height:200});
  });
  it('says what the image is for and when the work was made, then the provider\'s words, then its sources',()=>{
    expect(captionLines(item,{t,locale:'ru'})).toEqual([
      {part:'what',text:'Портрет автора · 1566'},
      {part:'title',text:'Portret van Marcus Tullius Cicero',lang:'nl'},
      {part:'inscribed',label:'Надпись на предмете',text:'Cicero'},
      {part:'series',label:'Серия',text:'Illustrium philosophorum et poetarum effigies',lang:'nl'},
      {part:'made',text:'printmaker: René Boyvin (mentioned on object)',lang:'en'},
      {part:'size',text:'175 × 121 мм'},
    ]);
    expect(captionLinks(item)).toEqual([
      {part:'record',text:'Rijksmuseum, Amsterdam · RP-P-2007-362',url:'https://id.rijksmuseum.nl/200488935'},
      {part:'rights',text:'Public Domain',url:PDM},
    ]);
  });
});
