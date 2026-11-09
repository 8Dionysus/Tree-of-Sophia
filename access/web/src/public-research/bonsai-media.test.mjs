import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {describe,expect,it} from 'vitest';
import {addMedia,mediaFromObservation,mediaIssues,readMedia} from '../../../../bonsai/lib/media.mjs';
import {MEDIA_HOSTS} from '../../../../bonsai/lib/media-hosts.mjs';
import {captionLines,captionLinks,fittedImage,fittedSize,imageSources,layerAddress,linkGroups} from './media-view.mjs';

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
  images:[{role:'primary',form:'image_service',rights_source:'https://data.rijksmuseum.nl/202488935?_profile=la-framed',image_service:{api_version:'3',service_id:'https://iiif.micr.io/qnEHd',width_pixels:2748,height_pixels:4044,info_fingerprint:{sha256:'b'.repeat(64),checked_at:'2026-11-08T05:12:58.0Z'}}}],
  ...overrides,
});
// A Met observation made with "files": true, cut to what the module reads; the file and its smaller copy are fingerprinted.
const metObservation=(overrides={})=>({
  schema_version:'tos_media_provider_observation_v1',provider:'met',requested_object:'333806',observed_at:'2026-11-08T06:28:52.978970949Z',
  requests:[{role:'object-record',sha256:'c'.repeat(64)}],
  object:{
    record_url:'https://www.metmuseum.org/art/collection/search/333806',repository:'Metropolitan Museum of Art, New York, NY',
    identifiers:[{scheme:'Met object ID',value:'333806'},{scheme:'Met object number',value:'19.27'}],
    titles:[{text:"'Nightmare'; an old woman carrying figures on her back",language:null,role:null}],
    makers:[{label:'Goya (Francisco de Goya y Lucientes)',role:'Artist',provider_uri:null,authority_uris:[]}],production_statements:[],
    dating:{statement:'ca. 1819–23',begin_year:1814,end_year:1828},classifications:['Drawings'],materials:['Brush and ink'],
    dimensions:[{kind:null,value:null,unit:null,statement:'9 1/4 x 5 11/16 in. (23.4 x 14.4 cm)'},{kind:'Sheet height',value:'23.4',unit:'cm',unit_uri:null},{kind:'Sheet width',value:'14.4',unit:'cm',unit_uri:null}],
    credit_lines:[{text:'Rogers Fund, 1919',language:null}],depicted:[],
  },
  rights:[{scope:'image',provider_field:'isPublicDomain',value:'true',statement_uri:null,label:null,stated_in:'https://collectionapi.metmuseum.org/public/collection/v1/objects/333806'}],
  images:[{role:'primary',form:'file',access_url:'https://images.metmuseum.org/CRDImages/dp/original/DP800144.jpg',image_service:null,file_url:'https://images.metmuseum.org/CRDImages/dp/original/DP800144.jpg',
    preview_url:'https://images.metmuseum.org/CRDImages/dp/web-large/DP800144.jpg',
    file_fingerprint:{url:'https://images.metmuseum.org/CRDImages/dp/original/DP800144.jpg',final_url:'https://images.metmuseum.org/CRDImages/dp/original/DP800144.jpg',media_type:'image/jpeg',byte_size:1374838,sha256:'3'.repeat(64),width_pixels:2414,height_pixels:3854},
    preview_fingerprint:{url:'https://images.metmuseum.org/CRDImages/dp/web-large/DP800144.jpg',final_url:'https://images.metmuseum.org/CRDImages/dp/web-large/DP800144.jpg',media_type:'image/jpeg',byte_size:57083,sha256:'2'.repeat(64),width_pixels:391,height_pixels:624}}],
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

  it('refuses an image without its service or its fingerprinted files, or whose rights do not let a stand show it',()=>{
    expect(()=>mediaFromObservation(observation({images:[{role:'primary',form:'file',file_url:'https://images.example/x.jpg',image_service:null,preview_url:null,file_fingerprint:null,preview_fingerprint:null}]}))).toThrow(/fingerprinted/);
    expect(()=>mediaFromObservation(observation({images:[]}))).toThrow(/no primary image/);
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

describe('a file image the stand keeps a copy of',()=>{
  const layers={schema:'tos.bonsai.layers.v1',paths:['media/'],entries:[{path:`media/${'2'.repeat(64)}.jpg`,sha256:'2'.repeat(64),bytes:57083},{path:`media/${'3'.repeat(64)}.jpg`,sha256:'3'.repeat(64),bytes:1374838}]};
  const attach=()=>addMedia({module:null,identities:{},observation:metObservation(),subject:'dwarf-weight#1',role:'motif',artifact:'european-drawing/metropolitan-museum-new-york/met-19-27',digest,bonsai:'zarathustra'});
  it('reads the Met in its own words: its title, its makers, the years its approximate dating states, its sheet in millimetres',()=>{
    const {work,image,workId}=mediaFromObservation(metObservation());
    expect(workId).toBe('met:19.27');
    expect(work.titles).toEqual([{text:"'Nightmare'; an old woman carrying figures on her back",language:null,role:'title'}]);
    expect(work.made).toEqual([{text:'Artist: Goya (Francisco de Goya y Lucientes)',language:null}]);
    expect(work.dating).toMatchObject({kind:'interval-assertion',certainty:'approximate',interval:{start:'1819',end:'1823'},source_wording:{text:'ca. 1819–23'}});
    expect(work.size_mm).toEqual([234,144]);
    expect(image).toMatchObject({api:'file',width:2414,height:3854,file:{sha256:'3'.repeat(64),bytes:1374838},preview:{sha256:'2'.repeat(64),width:391,height:624}});
  });
  it('names the representation after the file\'s bytes and holds when the layer store keeps both copies',()=>{
    const {module,identities,imageId}=attach();
    expect(identities.representations[imageId].tree).toBe(`tos.file.sha256.${'3'.repeat(64)}`);
    expect(mediaIssues(module,{identities,digest,layers})).toEqual([]);
    expect(mediaIssues(module,{identities,digest})).toContain(`image ${imageId}: file media requires a layer store`);
    const without={...layers,entries:layers.entries.slice(0,1)};
    expect(mediaIssues(module,{identities,digest,layers:without})).toContain(`image ${imageId}: the layer store holds no copy of its file (media/${'3'.repeat(64)}.jpg)`);
  });
  it('refuses a file image observed without its files',()=>{
    const light=metObservation();light.images[0].file_fingerprint=null;
    expect(()=>mediaFromObservation(light)).toThrow(/"files": true/);
  });
  it('opens an interval at the year a record says "in or after"',()=>{
    const later=mediaFromObservation(observation({object:{...observation().object,dating:{statement:'in or after 1868',begin_year:1868,end_year:1868}}}));
    expect(later.work.dating).toMatchObject({kind:'interval-assertion',interval:{start:'1868'}});
    expect(later.work.dating.interval.end).toBeUndefined();
  });
  it('shows the stand\'s own copy on the card and the file in the viewer; a public-domain flag links to the record',()=>{
    const {module}=attach(),[item]=readMedia(module).forSubject('dwarf-weight#1');
    expect(imageSources(item.image,{base:'/__research/'})).toEqual({src:`/__research/layers/sha256/${'2'.repeat(64)}`,srcset:'',full:{type:'image',url:`/__research/layers/sha256/${'3'.repeat(64)}`},crossOrigin:null});
    expect(layerAddress('a'.repeat(64),'/static')).toBe(`/static/layers/sha256/${'a'.repeat(64)}`);
    expect(captionLinks(item,{t:key=>({mediaPublicDomain:'Общественное достояние'})[key]??key})[1]).toEqual({part:'rights',text:'Общественное достояние',url:'https://www.metmuseum.org/art/collection/search/333806'});
  });
});

describe('an image of the National Gallery of Art',()=>{
  // As the bridge reads it from the Gallery's open data release at one commit: the record from its objects table, the image from IIIF 2.
  const release='https://raw.githubusercontent.com/NationalGalleryOfArt/opendata/9d3ba90ca8172c62c308a608373a957e9e4a1739';
  const nga={
    schema_version:'tos_media_provider_observation_v1',provider:'nga',requested_object:'1963.10.98',observed_at:'2026-11-08T06:55:40.416404721Z',
    requests:[{role:'image-service-info',sha256:'d'.repeat(64)}],
    datasets:[{path:'data/objects.csv',sha256:'0'.repeat(64)},{path:'data/published_images.csv',sha256:'1'.repeat(64)}],
    object:{record_url:'https://www.nga.gov/collection/art-object-page.46573.html',repository:'National Gallery of Art, Washington',
      identifiers:[{scheme:'NGA object ID',value:'46573'},{scheme:'NGA accession number',value:'1963.10.98'}],
      titles:[{text:'Woman with a Sunflower',language:null,role:null}],makers:[],production_statements:[{text:'Mary Cassatt',language:null}],
      dating:{statement:'c. 1905',begin_year:1905,end_year:1905},classifications:['Painting'],materials:['oil on canvas'],
      dimensions:[{kind:'Overall height',value:'92.1',unit:'cm'},{kind:'Overall width',value:'73.7',unit:'cm'}],credit_lines:[{text:'Chester Dale Collection',language:null}],depicted:[]},
    rights:[{scope:'image',provider_field:'openaccess',value:'1',statement_uri:null,label:null,stated_in:`${release}/data/published_images.csv`}],
    images:[{role:'primary',form:'image_service',image_service:{api_version:'2',service_id:'https://api.nga.gov/iiif/6ee124da-972a-4f02-83b8-7dc09c33f17c',width_pixels:17513,height_pixels:21806,info_fingerprint:{sha256:'e'.repeat(64),checked_at:'2026-11-08T06:55:40Z'}}}],
  };
  it('reads the release\'s record and the IIIF 2 service, and shows the image under the Gallery\'s open access flag',()=>{
    const {work,image,workId}=mediaFromObservation(nga);
    expect(workId).toBe('nga:1963.10.98');
    expect(work).toMatchObject({made:[{text:'Mary Cassatt',language:null}],size_mm:[921,737],observed:{record_sha256:'0'.repeat(64)}});
    expect(work.dating).toMatchObject({kind:'date-assertion',value:'1905',certainty:'approximate'});
    expect(image).toMatchObject({api:'iiif-image-2',service:'https://api.nga.gov/iiif/6ee124da-972a-4f02-83b8-7dc09c33f17c'});
    expect(imageSources(image).src).toBe('https://api.nga.gov/iiif/6ee124da-972a-4f02-83b8-7dc09c33f17c/full/!480,480/0/default.jpg');
    expect(captionLinks({work,image},{t:key=>({mediaOpenAccess:'Открытый доступ'})[key]??key})[1]).toEqual({part:'rights',text:'Открытый доступ',url:'https://www.nga.gov/collection/art-object-page.46573.html'});
    const closed=structuredClone(nga);closed.rights[0].value='0';
    expect(()=>mediaFromObservation(closed)).toThrow(/rights/);
  });
});

describe('the Zarathustra media',()=>{
  const read=path=>JSON.parse(readFileSync(new URL(`../../../../bonsai/zarathustra/${path}`,import.meta.url),'utf8'));
  const media=read('media.json'),identities=read('identities.json'),layers=read('layers.json');
  const core=['concepts','figures','proposed','links'].flatMap(name=>read(`core/${name}.json`)),chains=read('core/chains.json');
  const subjects=new Set([...core,...chains.flatMap(chain=>chain.states??[])].map(record=>record.id));
  it('holds its contract, binds core records only, and has a copy of every file in the layer store',()=>{
    expect(mediaIssues(media,{subjects,identities,digest,layers})).toEqual([]);
  });
  it('shows shared motifs and comparisons from open images: files of the Met, a Rijksmuseum sheet and paintings of the National Gallery of Art',()=>{
    expect(new Set(media.uses.map(use=>use.role))).toEqual(new Set(['motif','comparison']));
    expect(media.uses.length).toBe(320);
    // 192 stars and 60 links, each link with the registry's own selection for its meaning.
    const links=new Set(read('core/links.json').map(link=>link.id)),shown=new Set(media.uses.map(use=>use.subject));
    expect([...shown].filter(id=>!links.has(id)).length).toBe(192);
    expect([...shown].filter(id=>links.has(id)).length).toBe(60);
    expect(Object.keys(media.works).length).toBe(51);
    const terms=image=>({file:'met',['iiif-image-3']:'rijksmuseum',['iiif-image-2']:'nga'})[image.api];
    const open={met:{statement:null,field:'isPublicDomain',value:'true'},rijksmuseum:{statement:PDM},nga:{statement:null,field:'openaccess',value:'1'}};
    for(const image of Object.values(media.images)){
      expect(media.works[image.work].provider).toBe(terms(image));
      expect(image.rights).toMatchObject(open[terms(image)]);
    }
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

describe('the images on a link\'s card',()=>{
  const item=(id)=>({role:'comparison',image:{id},work:{id:id.split('/')[0]}});
  it('are the link\'s own where the bonsai binds images to the link',()=>{
    expect(linkGroups({own:[item('met:1/whole')],ends:[{label:'Лев',items:[item('met:2/whole')]}]})).toEqual([{label:null,items:[item('met:1/whole')]}]);
  });
  it('are else each end\'s under its name, an image both ends share once, an end without images left out',()=>{
    expect(linkGroups({ends:[{label:'Лев',items:[item('met:2/whole'),item('met:3/whole')]},{label:'Дитя',items:[item('met:3/whole'),item('met:4/whole')]},{label:'Осёл',items:[]}]})).toEqual([
      {label:'Лев',items:[item('met:2/whole'),item('met:3/whole')]},{label:'Дитя',items:[item('met:4/whole')]},
    ]);
    expect(linkGroups({ends:[{label:'Лев',items:[item('met:2/whole')]},{label:'Дитя',items:[item('met:2/whole')]}]})).toEqual([{label:'Лев',items:[item('met:2/whole')]}]);
    expect(linkGroups({ends:[]})).toEqual([]);
  });
  it('ask the image service for a copy fitted into the strip\'s box',()=>{
    expect(imageSources({api:'iiif-image-3',service:'https://iiif.micr.io/qnEHd'},{box:160}).src).toBe('https://iiif.micr.io/qnEHd/full/!160,160/0/default.jpg');
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
    expect(captionLinks(item,{t})).toEqual([
      {part:'record',text:'Rijksmuseum, Amsterdam · RP-P-2007-362',url:'https://id.rijksmuseum.nl/200488935'},
      {part:'rights',text:'Public Domain',url:PDM},
    ]);
  });
});
