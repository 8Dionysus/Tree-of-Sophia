// Names of the languages a word's descent passes through, by Wiktionary's codes.
const NAMES=new Map([
  ['ine-pro','праиндоевропейский','Proto-Indo-European'],['ine','индоевропейские','Indo-European'],
  ['gem-pro','прагерманский','Proto-Germanic'],['gem','германские','Germanic'],['gmw-pro','празападногерманский','Proto-West Germanic'],
  ['goh','древневерхненемецкий','Old High German'],['gmh','средневерхненемецкий','Middle High German'],['de-ear','ранненововерхненемецкий','Early New High German'],
  ['de','немецкий','German'],['gml','средненижненемецкий','Middle Low German'],['osx','древнесаксонский','Old Saxon'],['odt','древненидерландский','Old Dutch'],
  ['dum','средненидерландский','Middle Dutch'],['nl','нидерландский','Dutch'],['nds','нижненемецкий','Low German'],['nds-de','нижненемецкий','Low German'],
  ['frk','франкский','Frankish'],['ang','древнеанглийский','Old English'],['enm','среднеанглийский','Middle English'],['en','английский','English'],
  ['sco','шотландский','Scots'],['fy','западнофризский','West Frisian'],['ofs','древнефризский','Old Frisian'],['stq','затерландский фризский','Saterland Frisian'],
  ['non','древнескандинавский','Old Norse'],['is','исландский','Icelandic'],['fo','фарерский','Faroese'],['da','датский','Danish'],['sv','шведский','Swedish'],
  ['no','норвежский','Norwegian'],['nb','норвежский (букмол)','Norwegian Bokmål'],['nn','норвежский (нюношк)','Norwegian Nynorsk'],['got','готский','Gothic'],
  ['yi','идиш','Yiddish'],['lb','люксембургский','Luxembourgish'],['bar','баварский','Bavarian'],['gsw','алеманнский','Alemannic German'],
  ['hrx','хунсрикский','Hunsrik'],['af','африкаанс','Afrikaans'],['pdc','пенсильванский немецкий','Pennsylvania German'],
  ['la','латынь','Latin'],['la-lat','позднелатинский','Late Latin'],['la-med','средневековая латынь','Medieval Latin'],['la-vul','народная латынь','Vulgar Latin'],
  ['la-new','новолатинский','New Latin'],['la-ecc','церковная латынь','Ecclesiastical Latin'],['itc-pro','праиталийский','Proto-Italic'],
  ['grc','древнегреческий','Ancient Greek'],['grc-koi','греческий койне','Koine Greek'],['gkm','среднегреческий','Medieval Greek'],['el','греческий','Greek'],
  ['fro','старофранцузский','Old French'],['frm','среднефранцузский','Middle French'],['fr','французский','French'],['it','итальянский','Italian'],
  ['es','испанский','Spanish'],['pt','португальский','Portuguese'],['ca','каталанский','Catalan'],['roa-oit','староитальянский','Old Italian'],
  ['cel-pro','пракельтский','Proto-Celtic'],['cel','кельтские','Celtic'],['cel-gau','галльский','Gaulish'],['sga','древнеирландский','Old Irish'],
  ['ga','ирландский','Irish'],['cy','валлийский','Welsh'],['br','бретонский','Breton'],
  ['sla-pro','праславянский','Proto-Slavic'],['sla','славянские','Slavic'],['ine-bsl-pro','прабалтославянский','Proto-Balto-Slavic'],
  ['cu','старославянский','Old Church Slavonic'],['orv','древнерусский','Old East Slavic'],['zle-mru','старорусский','Middle Russian'],
  ['zle-ort','староукраинский и старобелорусский','Old Ruthenian'],['zle-ono','древненовгородский','Old Novgorodian'],['ru','русский','Russian'],
  ['uk','украинский','Ukrainian'],['be','белорусский','Belarusian'],['rue','русинский','Rusyn'],['pl','польский','Polish'],['zlw-opl','старопольский','Old Polish'],
  ['zlw-mpl','среднепольский','Middle Polish'],['cs','чешский','Czech'],['zlw-ocs','старочешский','Old Czech'],['sk','словацкий','Slovak'],
  ['hsb','верхнелужицкий','Upper Sorbian'],['dsb','нижнелужицкий','Lower Sorbian'],['csb','кашубский','Kashubian'],['szl','силезский','Silesian'],
  ['sh','сербохорватский','Serbo-Croatian'],['sl','словенский','Slovene'],['bg','болгарский','Bulgarian'],['mk','македонский','Macedonian'],
  ['lt','литовский','Lithuanian'],['lv','латышский','Latvian'],['prg','древнепрусский','Old Prussian'],['ltg','латгальский','Latgalian'],
  ['sa','санскрит','Sanskrit'],['sa-ved','ведийский санскрит','Vedic Sanskrit'],['iir-pro','праиндоиранский','Proto-Indo-Iranian'],
  ['ira-pro','праиранский','Proto-Iranian'],['ae','авестийский','Avestan'],['fa','персидский','Persian'],['fa-cls','классический персидский','Classical Persian'],
  ['xcl','древнеармянский','Old Armenian'],['hy','армянский','Armenian'],['sq','албанский','Albanian'],['hit','хеттский','Hittite'],
  ['txb','тохарский B','Tocharian B'],['xto','тохарский A','Tocharian A'],['os','осетинский','Ossetian'],['kmr','курманджи','Northern Kurdish'],
  ['he','иврит','Hebrew'],['hbo','древнееврейский','Biblical Hebrew'],['arc','арамейский','Aramaic'],['syc','классический сирийский','Classical Syriac'],
  ['ar','арабский','Arabic'],['sem','семитские','Semitic'],['egy','египетский','Egyptian'],
  ['trk-pro','пратюркский','Proto-Turkic'],['trk','тюркские','Turkic'],['otk','древнетюркский','Old Turkic'],['ota','османский','Ottoman Turkish'],
  ['tr','турецкий','Turkish'],['tt','татарский','Tatar'],['ba','башкирский','Bashkir'],['cv','чувашский','Chuvash'],['kk','казахский','Kazakh'],
  ['ky','киргизский','Kyrgyz'],['uz','узбекский','Uzbek'],['az','азербайджанский','Azerbaijani'],['crh','крымскотатарский','Crimean Tatar'],
  ['mn','монгольский','Mongolian'],['fi','финский','Finnish'],['et','эстонский','Estonian'],['hu','венгерский','Hungarian'],['se','северносаамский','Northern Sami'],
  ['urj-fin','прибалтийско-финские','Finnic'],['liv','ливский','Livonian'],['zh','китайский','Chinese'],['ja','японский','Japanese'],['eo','эсперанто','Esperanto'],
].map(([code,ru,en])=>[code,{ru,en}]));

/** The name of a language by its Wiktionary code in Russian or English, or null where the stand has none. */
export function languageName(code,russian){
  const names=NAMES.get(code);
  return names?(russian?names.ru:names.en):null;
}
