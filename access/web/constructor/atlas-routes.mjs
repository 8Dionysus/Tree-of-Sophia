/** Authored source-grounded reading routes; transitions retain atlas edge identity.
 * Own words are kept only for what a quotation cannot give: what happens at a place and who
 * speaks, how it joins the next place, the question it raises and the answer the book allows.
 * What a work itself says is never retold: a form cites the place instead ({ru, en, cite}),
 * and the stand shows the work's own text there. The map keeps the links the book itself makes
 * at a named place; everything else waits in atlas-held.mjs to be grown together. */
export const RESEARCH_ROUTES = [
  {
    "id": "fate-given-created",
    "title": {"ru":"Творящая воля и судьба","en":"The creative will and fate"},
    "description": {
      "ru": "На Блаженных островах Заратустра связывает созидание с превращением и прощанием, затем уточняет свои слова о судьбе.",
      "en": "On the Happy Isles, Zarathustra joins creation to transformation and parting, then corrects his words about fate."
    },
    "question": {
      "ru": "Что меняет поправка Заратустры о творящей воле и судьбе?",
      "en": "What changes when Zarathustra corrects his words about the creative will and fate?"
    },
    "steps": [
      {
        "nodeId": "demo:creation",
        "title": {"ru":"Созидание и прощание","en":"Creation and parting"},
        "body": {
          "ru": "Заратустра говорит о становлении и превращении, затем вспоминает, сколь часто ему приходилось прощаться. Это воспоминание подводит его к словам о творящей воле.",
          "en": "Zarathustra speaks of becoming and transformation, then recalls how often he has had to say farewell. That recollection leads him to speak of his creative will.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
          ]
        },
        "question": {"ru":"Почему речь о созидании приводит к прощанию?","en":"Why does a speech about creation lead to parting?"},
        "grounds": [
          {
            "ref": "z-happy-isles",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:fate",
        "title": {"ru":"Поправка о судьбе","en":"Correcting the words about fate"},
        "body": {
          "ru": "Заратустра сначала соединяет творящую волю со своей судьбой, затем останавливается и формулирует отношение заново. Поправка делает хотение самой воли предметом его высказывания.",
          "en": "Zarathustra first joins his creative will to his fate, then stops and puts their relation differently. The correction makes what the will wants the subject of his sentence.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
          ]
        },
        "question": {"ru":"Какую судьбу хочет его воля?","en":"What kind of fate does his will want?"},
        "grounds": [
          {
            "ref": "z-happy-isles",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "demo:creation",
        "to": "demo:fate",
        "edgeId": "atlas:creation:develops:fate",
        "body": {
          "ru": "Воспоминание о прощаниях ведёт Заратустру от превращения к поправке о судьбе.",
          "en": "The memory of partings leads Zarathustra from transformation to his correction about fate."
        }
      }
    ],
    "conclusion": {
      "ru": "Судьба названа внутри речи о творящей воле; в поправке именно воля хочет такой судьбы. Как она относится к тому, что уже совершилось, можно далее спросить у речи «Об избавлении».",
      "en": "Fate is named within the speech about the creative will; in the correction, the will wants just such a fate. What willing can do with what has already happened leads on to “On Redemption.”"
    },
    "investigation": {
      "startingPoint": {
        "ru": "Проследите фразы о становлении, превращении и прощании до поправки о судьбе.",
        "en": "Follow the sentences on becoming, transformation and parting up to the correction about fate."
      },
      "stakes": {
        "ru": "В первой формулировке судьба стоит рядом с волей; во второй воля хочет судьбы. Этот поворот задаёт вопрос об их отношении.",
        "en": "The first formulation places fate beside the will; the second has the will wanting fate. The turn raises a question about their relation."
      },
      "carryForward": {
        "ru": "Прочитайте следом «Об избавлении»: какой вопрос о прошлом там ставит воля?",
        "en": "Read “On Redemption” next: what question does the will face there about the past?"
      }
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-104dd6450268fddea57462ac8c13cb9a"]}
        ]
      }
    ]
  },
  {
    "id": "moment-time",
    "title": {"ru":"У ворот «Мгновение»","en":"At the gateway “Moment”"},
    "description": {
      "ru": "Заратустра показывает карлику две дороги, сходящиеся у ворот; вопрос об их противоречии получает ответ о круге.",
      "en": "Zarathustra shows the dwarf two paths meeting at a gateway; his question about their opposition receives an answer about a circle."
    },
    "question": {
      "ru": "Как разговор у ворот связывает противоположные дороги со временем?",
      "en": "How does the conversation at the gateway relate the opposing paths to time?"
    },
    "steps": [
      {
        "nodeId": "moment",
        "title": {"ru":"Имя места встречи","en":"Naming the meeting place"},
        "body": {
          "ru": "В рассказе Заратустры ворота получают имя «Мгновение». От этого места он обращает внимание спутника на дороги.",
          "en": "In Zarathustra’s account the gateway is named “Moment.” From this place he directs his companion’s attention to the paths.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
          ]
        },
        "question": {
          "ru": "Что имя ворот добавляет к картине двух дорог?",
          "en": "What does the gateway’s name add to the picture of the two paths?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:gateway",
        "title": {"ru":"Ворота и спутник","en":"The gateway and the companion"},
        "body": {
          "ru": "Карлик спрыгивает с плеча Заратустры и садится у ворот. Теперь Заратустра может показать ему место встречи дорог.",
          "en": "The dwarf jumps from Zarathustra’s shoulder and sits beside the gateway. Zarathustra can now show him where the paths meet.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-dff926efa83f3da3ee1d67d9a3e0538b","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
          ]
        },
        "question": {
          "ru": "Почему Заратустра показывает дороги именно здесь?",
          "en": "Why does Zarathustra point out the paths here?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-dff926efa83f3da3ee1d67d9a3e0538b","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:two-paths",
        "title": {"ru":"Назад и вперёд","en":"Back and forward"},
        "body": {
          "ru": "Одна дорога уходит назад, другая вперёд; у ворот они встречаются. Заратустра спрашивает, будут ли они вечно противоречить друг другу.",
          "en": "One path runs back and the other forward; they meet at the gateway. Zarathustra asks whether they will contradict each other forever.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]}
          ]
        },
        "question": {
          "ru": "Что означает вопрос об их вечном противоречии?",
          "en": "What is at stake in asking whether their opposition lasts forever?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:circle",
        "title": {"ru":"Ответ карлика","en":"The dwarf’s answer"},
        "body": {
          "ru": "Карлик отвечает образом круга. Заратустра сердится на этот ответ и продолжает расспрашивать о прошлом и возвращении самого мгновения.",
          "en": "The dwarf answers with the image of a circle. Zarathustra objects and goes on to ask about the past and the return of the moment itself.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
          ]
        },
        "question": {
          "ru": "Чем вопросы Заратустры продолжают ответ карлика?",
          "en": "How do Zarathustra’s questions carry the dwarf’s answer further?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "moment",
        "to": "demo:gateway",
        "edgeId": "atlas:moment:supports:gateway",
        "body": {
          "ru": "Имя ворот делает место встречи дорог «Мгновением».",
          "en": "The gateway’s name makes the meeting place “Moment.”"
        }
      },
      {
        "from": "demo:gateway",
        "to": "demo:two-paths",
        "edgeId": "atlas:gateway:supports:two-paths",
        "body": {
          "ru": "От ворот взгляд следует по дорогам в противоположные стороны.",
          "en": "From the gateway, the gaze follows the paths in opposite directions."
        }
      },
      {
        "from": "demo:two-paths",
        "to": "demo:circle",
        "edgeId": "atlas:two-paths:supports:circle",
        "body": {
          "ru": "На вопрос о противоречии карлик отвечает кругом времени.",
          "en": "The dwarf answers the question about opposition with a circle of time."
        }
      }
    ],
    "conclusion": {
      "ru": "У ворот вопрос о двух дорогах получает ответ карлика, а затем Заратустра спрашивает о возвращении ворот и самих собеседников. Его вопросы оставляют читателю задачу проследить собственный ход рассуждения Заратустры.",
      "en": "At the gateway the dwarf answers the question about the paths; Zarathustra then asks about the return of the gateway and the speakers themselves. His questions invite the reader to follow his own line of thought."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Представьте место, которое Заратустра показывает карлику, и проследите направления дорог.",
        "en": "Picture the place Zarathustra shows the dwarf and follow each path’s direction."
      },
      "stakes": {
        "ru": "Ответ карлика и последующие вопросы Заратустры принадлежат разным голосам одного разговора.",
        "en": "The dwarf’s answer and Zarathustra’s subsequent questions belong to different voices in the same conversation."
      },
      "carryForward": {
        "ru": "Прочитайте вопросы после ответа карлика: как Заратустра включает в них ворота и собеседников?",
        "en": "Read the questions after the dwarf’s answer: how does Zarathustra bring the gateway and the speakers into them?"
      }
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-dff926efa83f3da3ee1d67d9a3e0538b","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ]
  },
  {
    "id": "repetition-difference",
    "title": {"ru":"Та же самая жизнь","en":"The very same life"},
    "description": {
      "ru": "Звери воображают, что Заратустра скажет перед смертью, и называют землю и спутников его будущей жизни.",
      "en": "The animals imagine what Zarathustra will say before dying, naming the earth and companions of his life to come."
    },
    "question": {
      "ru": "Что делает возвращающуюся жизнь той же самой в речи зверей?",
      "en": "What makes the returning life the same in the animals’ speech?"
    },
    "steps": [
      {
        "nodeId": "same-life",
        "title": {"ru":"Воображаемые слова Заратустры","en":"Words imagined for Zarathustra"},
        "body": {
          "ru": "Звери обращаются к Заратустре и воображают его последние слова. В этой вложенной речи он говорит о возвращении к своей жизни и своим спутникам.",
          "en": "The animals address Zarathustra and imagine his last words. In the speech they put in his mouth he speaks of returning to his life and companions.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
          ]
        },
        "question": {"ru":"Кто здесь произносит слова о возвращении?","en":"Who speaks the words about returning here?"},
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:identity",
        "title": {"ru":"Эта земля и эти спутники","en":"This earth and these companions"},
        "body": {
          "ru": "Указания на эту землю и этих спутников придают слову «та же» конкретность. Речь зверей позволяет спросить, как возвращение той же жизни соотносится с переменой внутри неё.",
          "en": "The references to this earth and these companions give “the same” concrete content. The animals’ speech invites the question of how the return of the same life relates to change within it.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
          ]
        },
        "question": {"ru":"Что именно узнаётся как то же самое?","en":"What exactly is recognised as the same?"},
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "same-life",
        "to": "demo:identity",
        "edgeId": "atlas:same-life:supports:identity",
        "body": {
          "ru": "Вложенная речь называет землю и спутников, через которых раскрывается «та же жизнь».",
          "en": "The imagined speech names the earth and companions through which “the same life” takes shape."
        }
      }
    ],
    "conclusion": {
      "ru": "Звери дают возвращению узнаваемые черты этой жизни. Вопрос о том, как её тождество сочетается с переменой, ведёт к их словам о становлении и кругообороте вещей.",
      "en": "The animals give recurrence the recognisable features of this life. The question of how its sameness accommodates change leads to their words about becoming and the cycle of things."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Отметьте, где звери обращаются к Заратустре и где начинают говорить от его имени.",
        "en": "Notice where the animals address Zarathustra and where they begin speaking in his imagined voice."
      },
      "stakes": {
        "ru": "Повторённые указания на эту землю и спутников позволяют увидеть, какую жизнь называет речь.",
        "en": "Repeated references to this earth and these companions let us see which life the speech names."
      },
      "carryForward": {
        "ru": "Сопоставьте эту речь с песней зверей о том, как вещи уходят и приходят снова.",
        "en": "Read this speech alongside the animals’ song of things departing and returning."
      }
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
        ]
      }
    ]
  },
  {
    "id": "affirmation-pain-action",
    "title": {"ru":"Радость, горе и возвращение","en":"Joy, sorrow and return"},
    "description": {
      "ru": "В «Пьяной песне» Заратустра ведёт от согласия с одной радостью к согласию со всем горем и желанию возвращения.",
      "en": "In “The Drunken Song,” Zarathustra moves from saying yes to one joy to affirming all sorrow and wanting return."
    },
    "question": {
      "ru": "Почему «да» одной радости влечёт за собой горе и возвращение всего?",
      "en": "Why does saying yes to one joy carry sorrow and the return of everything with it?"
    },
    "steps": [
      {
        "nodeId": "demo:affirmation",
        "title": {"ru":"«Да» одной радости","en":"A yes to one joy"},
        "body": {
          "ru": "Заратустра обращается к друзьям с вопросом об одной радости. С этого частного согласия начинается движение его речи ко всем вещам.",
          "en": "Zarathustra asks his friends whether they have ever said yes to one joy. That particular assent begins his movement toward all things.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
          ]
        },
        "question": {
          "ru": "Как согласие с одной радостью открывает вопрос о целом?",
          "en": "How does assent to one joy open a question about the whole?"
        },
        "grounds": [
          {
            "ref": "z-drunken-song",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:joy-pain",
        "title": {"ru":"Связанность радости и горя","en":"Joy and sorrow entwined"},
        "body": {
          "ru": "Заратустра говорит о сцепленных и переплетённых вещах: потому согласие с радостью охватывает и горе. В этой связности он находит основание перехода от одного переживания ко всем.",
          "en": "Zarathustra speaks of things chained and entwined: assent to joy therefore embraces sorrow too. Their connectedness grounds his movement from one experience to all things.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
          ]
        },
        "question": {
          "ru": "Как связанность вещей ведёт от радости к горю?",
          "en": "How does the connectedness of things lead from joy to sorrow?"
        },
        "grounds": [
          {
            "ref": "z-drunken-song",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:recurrence",
        "title": {"ru":"«Пройди, но вернись»","en":"“Pass away, but come back”"},
        "body": {
          "ru": "В конце этого движения Заратустра обращается и к горю. Его обращение держит вместе желание ухода и возвращения, а стремление радости к вечности даёт ему основание.",
          "en": "At the end of this movement Zarathustra addresses sorrow too. His appeal holds passing away and return together, grounded in joy’s desire for eternity.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
          ]
        },
        "question": {
          "ru": "Как желание вечности меняет обращение к горю?",
          "en": "How does the desire for eternity change the appeal to sorrow?"
        },
        "grounds": [
          {
            "ref": "z-drunken-song",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "demo:affirmation",
        "to": "demo:joy-pain",
        "edgeId": "atlas:affirmation:supports:joy-pain",
        "body": {
          "ru": "Заратустра соединяет согласие с радостью со всем горем через связанность вещей.",
          "en": "Zarathustra joins assent to joy with all sorrow through the connectedness of things."
        }
      },
      {
        "from": "demo:joy-pain",
        "to": "demo:recurrence",
        "edgeId": "atlas:joy-pain:supports:recurrence",
        "body": {
          "ru": "Связанность ведёт к обращению, в котором горе просят пройти и вернуться.",
          "en": "That connectedness leads to an appeal asking sorrow both to pass and to return."
        }
      }
    ],
    "conclusion": {
      "ru": "Движение песни держится на связанности вещей и желании радости длиться вечно. Из одного «да» Заратустра выводит обращение ко всему, включая горе.",
      "en": "The song’s movement rests on the connectedness of things and joy’s desire for eternity. From one yes Zarathustra draws an appeal to everything, sorrow included."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Прочитайте вопрос об одной радости и следите, как меняется масштаб речи.",
        "en": "Read the question about one joy and follow how the scale of the speech changes."
      },
      "stakes": {
        "ru": "Отношение радости к горю определяется здесь связью всех вещей и желанием вечности.",
        "en": "Here the relation between joy and sorrow is shaped by the connection among things and the desire for eternity."
      },
      "carryForward": {
        "ru": "Вернитесь к словам о сцепленных вещах: какую работу они делают в переходе от одной радости ко всему?",
        "en": "Return to the words about things being chained: what work do they do in the move from one joy to everything?"
      }
    },
    "grounds": [
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ae02bb0c97b4ccccfef8c26220bd3b00"]}
        ]
      }
    ]
  },
  {
    "id": "reading-voices",
    "title": {"ru":"Песня и судьба учителя","en":"The song and the teacher’s fate"},
    "description": {
      "ru": "Звери поют выздоравливающему Заратустре о возвращении вещей, затем называют его учителем возвращения и говорят о его судьбе.",
      "en": "The animals sing to the convalescent Zarathustra of things returning, then call him the teacher of recurrence and speak of his fate."
    },
    "question": {
      "ru": "Как песня зверей приводит их к судьбе и задаче Заратустры?",
      "en": "How does the animals’ song lead them to Zarathustra’s fate and task?"
    },
    "steps": [
      {
        "nodeId": "all-things",
        "title": {"ru":"Песня вещей","en":"The song of things"},
        "body": {
          "ru": "Звери начинают песню об уходе и возвращении вещей. Заратустра улыбается и отвечает вопросом о шарманочной песне, вспоминая своё испытание.",
          "en": "The animals begin a song about things departing and returning. Zarathustra smiles and answers with a question about a barrel-organ song, recalling his ordeal.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
          ]
        },
        "question": {"ru":"Как его ответ меняет звучание их песни?","en":"How does his reply change the way their song sounds?"},
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:animals",
        "title": {"ru":"Звери и больной","en":"The animals and the convalescent"},
        "body": {
          "ru": "Звери заботятся о Заратустре и говорят с ним после болезни. Их песня получает ответ человека, который ещё устал от собственного избавления.",
          "en": "The animals care for Zarathustra and speak to him after his illness. Their song receives an answer from someone still exhausted by his own deliverance.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
          ]
        },
        "question": {
          "ru": "Что открывает ответ Заратустры о расстоянии между песней и его переживанием?",
          "en": "What does Zarathustra’s reply reveal about the distance between the song and his experience?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:recurrence",
        "title": {"ru":"Имя учения","en":"Naming the teaching"},
        "body": {
          "ru": "Позже звери называют Заратустру учителем вечного возвращения и излагают то, чему, по их словам, он учит. Теперь речь идёт о нём самом и его месте в этой мысли.",
          "en": "Later the animals call Zarathustra the teacher of eternal recurrence and describe what they say he teaches. Their speech now turns to him and his place in that thought.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
          ]
        },
        "question": {
          "ru": "Как звери связывают возвращение вещей с учителем?",
          "en": "How do the animals connect the return of things with a teacher?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:fate",
        "title": {"ru":"Судьба учителя","en":"The teacher’s fate"},
        "body": {
          "ru": "Звери называют учительство судьбой Заратустры. Они обращаются к нему как к тому, кому предстоит первым учить возвращению.",
          "en": "The animals name Zarathustra’s teaching his fate. They address him as the one who must be the first to teach recurrence.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
          ]
        },
        "question": {
          "ru": "Что добавляет слово «судьба» к имени учителя?",
          "en": "What does the word “fate” add to the name of teacher?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:task",
        "title": {"ru":"Опасность учительства","en":"The danger of teaching"},
        "body": {
          "ru": "Назвав его судьбу, звери спрашивают о её опасности и связывают её с болезнью. Их вопрос оставляет перед читателем цену учительства.",
          "en": "Having named his fate, the animals ask about its danger and connect it to his illness. Their question leaves the cost of teaching before the reader.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
          ]
        },
        "question": {"ru":"В чём, по словам зверей, опасность этой судьбы?","en":"What danger do the animals see in this fate?"},
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "all-things",
        "to": "demo:animals",
        "edgeId": "atlas:all-things:supports:animals",
        "body": {
          "ru": "Песню зверей сопровождает ответ Заратустры, ещё больного после своего избавления.",
          "en": "The animals’ song meets Zarathustra’s reply while he is still ill from his deliverance."
        }
      },
      {
        "from": "demo:animals",
        "to": "demo:recurrence",
        "edgeId": "atlas:animals:supports:recurrence",
        "body": {
          "ru": "От песни звери переходят к тому, как они называют учение Заратустры.",
          "en": "The animals move from song to naming Zarathustra’s teaching."
        }
      },
      {
        "from": "demo:recurrence",
        "to": "demo:fate",
        "edgeId": "atlas:recurrence:supports:fate",
        "body": {
          "ru": "Имя учителя звери соединяют со словом «судьба».",
          "en": "The animals join the name of teacher to the word “fate.”"
        }
      },
      {
        "from": "demo:fate",
        "to": "demo:task",
        "edgeId": "atlas:fate:supports:task",
        "body": {
          "ru": "Назвав судьбу, звери спрашивают, какова её опасность.",
          "en": "Having named his fate, the animals ask what danger it holds."
        }
      }
    ],
    "conclusion": {
      "ru": "Звери проводят путь от песни о вещах к судьбе первого учителя возвращения. Ответ Заратустры о шарманочной песне сохраняет различие между их словами и пережитым им испытанием.",
      "en": "The animals move from a song about things to the fate of the first teacher of recurrence. Zarathustra’s reply about a barrel-organ song preserves the distance between their words and his ordeal."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Отметьте смену говорящего: песня зверей, ответ Заратустры, новая речь зверей.",
        "en": "Mark the changes of speaker: the animals’ song, Zarathustra’s reply, then the animals speaking again."
      },
      "stakes": {
        "ru": "Имя учителя и его судьбу произносят звери; ответ Заратустры позволяет услышать их песню рядом с его болезнью.",
        "en": "The animals name the teacher and his fate; Zarathustra’s reply lets us hear their song beside his illness."
      },
      "carryForward": {
        "ru": "Прочитайте воображаемые зверями последние слова Заратустры: как их голос вкладывает речь в уста другого?",
        "en": "Read the last words the animals imagine for Zarathustra: how does their voice place speech in another’s mouth?"
      }
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      }
    ]
  },
  {
    "id": "image-experience",
    "title": {"ru":"Пастух, укус, смех","en":"The shepherd, the bite, the laughter"},
    "description": {
      "ru": "Видение Заратустры движется от задыхающегося пастуха к укусу змеи и смеху преображённого.",
      "en": "Zarathustra’s vision moves from a choking shepherd to the bite of the serpent and the laughter of a transformed figure."
    },
    "question": {
      "ru": "Что происходит между удушьем пастуха и смехом, который слышит Заратустра?",
      "en": "What happens between the shepherd’s choking and the laughter Zarathustra hears?"
    },
    "steps": [
      {
        "nodeId": "demo:shepherd",
        "title": {"ru":"Змея в горле","en":"The serpent in the throat"},
        "body": {
          "ru": "Заратустра видит юного пастуха, задыхающегося от змеи. Он пытается вырвать её рукой, затем кричит пастуху откусить ей голову.",
          "en": "Zarathustra sees a young shepherd choking on a serpent. He tries to pull it out by hand, then cries out for the shepherd to bite off its head.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
          ]
        },
        "question": {
          "ru": "Что меняется между попыткой Заратустры и его криком?",
          "en": "What changes between Zarathustra’s attempt and his cry?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:bite",
        "title": {"ru":"Действие пастуха","en":"The shepherd acts"},
        "body": {
          "ru": "Пастух следует крику Заратустры, откусывает голову змеи и выплёвывает её. После этого он вскакивает.",
          "en": "The shepherd follows Zarathustra’s cry, bites off the serpent’s head and spits it out. Then he leaps up.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
          ]
        },
        "question": {"ru":"Что делает сам пастух после крика?","en":"What does the shepherd himself do after the cry?"},
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:laughter",
        "title": {"ru":"Смех преображённого","en":"The transformed figure laughs"},
        "body": {
          "ru": "Рассказчик видит смеющегося преображённого и говорит о тоске по этому смеху. Кто этот пастух и что означает видение, остаётся вопросом, который сам Заратустра задаёт перед укусом.",
          "en": "The narrator sees a transformed figure laughing and speaks of his longing for that laughter. Who the shepherd is and what the vision means remain questions Zarathustra himself asks before the bite.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-38407ce82a4703428377d06e4af5c240"]}
          ]
        },
        "question": {
          "ru": "Кто смеётся после укуса и почему этот смех остаётся с Заратустрой?",
          "en": "Who laughs after the bite, and why does that laughter stay with Zarathustra?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-38407ce82a4703428377d06e4af5c240"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "demo:shepherd",
        "to": "demo:bite",
        "edgeId": "atlas:shepherd:develops:bite",
        "body": {
          "ru": "Рука Заратустры не может вырвать змею; его крик ведёт к действию пастуха.",
          "en": "Zarathustra’s hand cannot pull out the serpent; his cry leads to the shepherd’s action."
        }
      },
      {
        "from": "demo:bite",
        "to": "demo:laughter",
        "edgeId": "atlas:bite:develops:laughter",
        "body": {
          "ru": "После укуса и плевка пастух вскакивает, и рассказчик видит смеющегося преображённого.",
          "en": "After biting and spitting, the shepherd leaps up, and the narrator sees a transformed figure laughing."
        }
      }
    ],
    "conclusion": {
      "ru": "Сцена показывает последовательность: удушье, тщетную попытку помощи, крик, укус и смех. Вопрос Заратустры о пастухе оставляет смысл видения открытым для дальнейшего чтения.",
      "en": "The scene unfolds through choking, an unsuccessful attempt to help, a cry, a bite and laughter. Zarathustra’s question about the shepherd leaves the meaning of the vision open to further reading."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Следите за действиями двух людей: что делает Заратустра и что делает пастух?",
        "en": "Follow the actions of both figures: what does Zarathustra do, and what does the shepherd do?"
      },
      "stakes": {
        "ru": "Крик соединяет попытку помочь с действием самого пастуха; смех появляется после укуса.",
        "en": "The cry joins the attempt to help to the shepherd’s own action; laughter follows the bite."
      },
      "carryForward": {
        "ru": "Вернитесь к вопросу «Кто пастух?» перед укусом и прочитайте вместе с ответом Заратустры зверям после болезни.",
        "en": "Return to “Who is the shepherd?” before the bite, then read it alongside Zarathustra’s reply to the animals after his illness."
      }
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-38407ce82a4703428377d06e4af5c240"]}
        ]
      }
    ]
  }
];
