# Plato, Apology: Spanish card alignment withheld

Reviewed against Antiquity alignment at main `04d82d39e1314e4ce99fbc78de9056a7b7644948`, original `retained-apology-grc2-6077e9014cf8` revision `aa3268383fa165879f098f106c92b87c5bae3bb8eb9c88be55f8d5cfa591191e`, Spanish witness `retained-apology-es-azcarate1871` revision `b91d88e28dc6383145bbab4d33f3670b7ab9991af9ceaf0a287887f24c163685`. Greek ranges use code-point offsets in corpus units; descriptions of Spanish are what the stored alignment returns.

| Original unit and range | Original subject | Incorrect Spanish subject |
| --- | --- | --- |
| `tos.text-unit.sid-191e713c00564ecca0424f7d4a866e90`, 1440–1538, Stephanus 24 | Socrates asks Meletus whether he cares about improving the young | A later question about who improves the young |
| `tos.text-unit.sid-7b2fe5a62f1f40e0b6eebd51fd3a9d02`, 457–575, Stephanus 28 | Whether Socrates should be ashamed of conduct risking death | The following account of Achilles at Troy |
| `tos.text-unit.sid-a106008ce6e04dcf81c7d0de25803c2d`, 912–1009, Stephanus 37 | Whether imprisonment is an alternative penalty | A complaint that the trial was too short |

The retained text also omits material visible in the source, including the Homeric verse and intervening prose on Wikisource [pages 116](https://es.wikisource.org/wiki/P%C3%A1gina:Obras_completas_de_Plat%C3%B3n_-_Tomo_I_(1871).djvu/116) and [117](https://es.wikisource.org/wiki/P%C3%A1gina:Obras_completas_de_Plat%C3%B3n_-_Tomo_I_(1871).djvu/117). The point at which this material was lost is not established. The reviewed examples establish specific errors, not that every pair is wrong. A trial extraction containing the missing text still produced mismatched pairs, so it has not been substituted for the retained source.

The scope temporarily withholds the entire Spanish witness's card alignment, bound to both exact revisions. This can also hide correct pairs; the corpus witness remains available for reading. Restore alignment after a separately sourced correction and structural alignment have been reviewed; do not select individual card passages by hand.

## English Fowler layers

The card selects the first English version with a matching pair. In Greek unit `tos.text-unit.sid-7d03459578bd4415ba5b16362b4422cb` (Stephanus 38), the range 716–790 says Socrates might pay **one mina**; `perseus-fowler-1914-en` revision `e9aad012204bd47d908d9fc119e0d81ad924388d362554835d18076b85f15c24` returns the later proposal of **thirty minas**. The alternative retained Fowler layer `retained-apology-eng2-09ae91e79022` revision `540f83118151bf91d96c84316f0ddb77594ed6019c2db458a2e12db9a3285fc9` returns “I might perhaps pay a mina of silver.” Both are derived from the same [Perseus TEI source](https://raw.githubusercontent.com/PerseusDL/canonical-greekLit/341e309c821d5eca8c976bebca77c28b10bad58f/data/tlg0059/tlg002/tlg0059.tlg002.perseus-eng2.xml), with distinct reading layers and alignments.

Withholding the first layer's card alignment lets the existing reader select the alternative. All 53 unique currently cited Greek ranges have a nonempty alternative pair in the local snapshot; comparison of their subjects against Russian context, and several against the Greek, found the alternative more precise in several ranges; this is not a complete scholarly verification. Both English witnesses remain available for reading.
