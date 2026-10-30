/** Authored source-grounded reading routes; transitions retain atlas edge identity.
 * Own words are kept only for what a quotation cannot give (a reading, a
 * reconstructed argument, a distinction, a connection, a question, a method,
 * context). What a work itself says is never retold: a form cites the place
 * instead ({ru, en, cite}), and the stand shows the work's own text there —
 * the original and, through the alignment of the witnesses, the reader's
 * language (bonsai/zarathustra/witnesses.json, fragments.json). */
export const RESEARCH_ROUTES = [
  {
    "id": "fate-given-created",
    "title": {"ru":"Судьба: данная и создаваемая","en":"Fate: given and made"},
    "description": {
      "ru": "Начните с двух способов назвать судьбу своей: воля хочет её у Заратустры, поступки соединяются памятью у Камю. Через спуск Сизифа и вопрос демона маршрут приходит к «Об избавлении»: что может творящая воля перед тем, что уже было?",
      "en": "Begin with two ways of calling fate one’s own: willing wants it in Zarathustra, memory joins actions into it in Camus. Through Sisyphus’s descent and the demon’s question, the route reaches “Redemption”: what can creative willing do before what has already been?"
    },
    "question": {
      "ru": "Как действие входит в судьбу и что воля может сделать с уже бывшим?",
      "en": "How does action enter fate, and what can willing do with what has already been?"
    },
    "steps": [
      {
        "nodeId": "demo:fate",
        "title": {"ru":"Что значит «моя судьба»?","en":"What makes fate mine?"},
        "body": {
          "ru": "Прочитайте рядом эти два притяжательных жеста: хотеть удела и узнавать связность собственного пути.",
          "en": "Read these two gestures of appropriation together: wanting a lot and recognising one’s path as connected.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]},
            {"w":"le-mythe-de-sisyphe","loc":"joy"}
          ]
        },
        "question": {"ru":"Как изменяется «моё» между волей и памятью?","en":"How does “mine” change between willing and memory?"},
        "grounds": [
          {
            "ref": "z-happy-isles",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
            ]
          },
          {"ref":"camus-sisyphus","cite":[{"w":"le-mythe-de-sisyphe","loc":"joy"}]}
        ]
      },
      {
        "nodeId": "demo:camus",
        "title": {"ru":"Поступки, собранные взглядом","en":"Actions gathered by a gaze"},
        "body": {
          "ru": "Формулировка Камю о взгляде памяти стоит внутри возвращения Сизифа к камню, поэтому личная связность возникает вместе с продолжающимся трудом. Уточните, что память здесь делает с совершённым.",
          "en": "Camus’s formulation about memory’s gaze stands within Sisyphus’s return to the stone, so personal connectedness arises alongside continuing labour. Specify what memory does here with what has been done.",
          "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"},{"w":"le-mythe-de-sisyphe","loc":"joy"}]
        },
        "question": {
          "ru": "Как взгляд на последовательность поступков позволяет узнать судьбу своей?",
          "en": "How does surveying a sequence of actions allow fate to be recognised as one’s own?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"},{"w":"le-mythe-de-sisyphe","loc":"joy"}]
          }
        ]
      },
      {
        "nodeId": "demo:sisyphus",
        "title": {"ru":"Спуск как час сознания","en":"The descent as consciousness’s hour"},
        "body": {
          "ru": "Камень вновь ждёт внизу; изменение нужно искать в отношении героя к следующему усилию.",
          "en": "The stone awaits below again; the change lies in the hero’s relation to the next effort.",
          "cite": [{"w":"le-mythe-de-sisyphe","loc":"return"},{"w":"le-mythe-de-sisyphe","loc":"tragic"}]
        },
        "question": {
          "ru": "Почему ясность возникает именно между двумя подъёмами?",
          "en": "Why does lucidity arise precisely between two ascents?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "cite": [{"w":"le-mythe-de-sisyphe","loc":"return"},{"w":"le-mythe-de-sisyphe","loc":"tragic"}]
          }
        ]
      },
      {
        "nodeId": "demo:agency",
        "title": {"ru":"Участие в повторяемом","en":"Participating in what repeats"},
        "body": {
          "ru": "Переход меняет форму участия: от обозрения труда к мысли, которая способна преобразить действующего.",
          "en": "The transition changes participation’s form: from surveying labour to a thought capable of transforming the agent.",
          "cite": [
            {"w":"le-mythe-de-sisyphe","loc":"return"},
            {"w":"le-mythe-de-sisyphe","loc":"tragic"},
            {"w":"le-mythe-de-sisyphe","loc":"joy"},
            {"w":"die-froehliche-wissenschaft","loc":"341"}
          ]
        },
        "question": {
          "ru": "Что меняется, когда знание повтора становится вопросом при действии?",
          "en": "What changes when knowledge of repetition becomes a question accompanying action?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "cite": [
              {"w":"le-mythe-de-sisyphe","loc":"return"},
              {"w":"le-mythe-de-sisyphe","loc":"tragic"},
              {"w":"le-mythe-de-sisyphe","loc":"joy"}
            ]
          },
          {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
        ]
      },
      {
        "nodeId": "demo:decision",
        "title": {"ru":"Решение под тяжестью вопроса","en":"Decision under the question’s weight"},
        "body": {
          "ru": "Прочитайте переход в §341 внимательно: желание повторения должно изменить отношение к совершаемому при сохранении строгого условия того же.",
          "en": "Read the transition in §341 closely: wanting recurrence is to change one’s relation to acting while preserving the same-life condition.",
          "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
        },
        "question": {
          "ru": "Как вопрос о неизменном повторе может преобразить совершающийся поступок?",
          "en": "How can a question about unaltered recurrence transform an action being performed?"
        },
        "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}]
      },
      {
        "nodeId": "demo:counterfactual",
        "title": {"ru":"Перед словом «было»","en":"Before the word “was”"},
        "body": {
          "ru": "Рядом с демоном вопрос «иначе» становится точнее. Воображение другого поступка и отношение к уже бывшему имеют разные направления, и дальнейшая речь Заратустры занимается вторым.",
          "en": "Beside the demon, “otherwise” becomes more precise. Imagining another act and relating to what has been point in different directions; Zarathustra’s ensuing speech concerns the latter.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"],"s":72}
          ]
        },
        "question": {
          "ru": "Чего воля хочет от прошлого, когда больше не может его изменить?",
          "en": "What does willing want from the past when it can no longer change it?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"],"s":1}
            ]
          },
          {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
        ]
      },
      {
        "nodeId": "demo:responsibility",
        "title": {"ru":"От бессилия к обвинению","en":"From powerlessness to accusation"},
        "body": {
          "ru": "Проследите, как бессилие перед временем становится у Заратустры местью, а затем объяснением существования через наказание. Вопрос ответственности здесь начинается с происхождения вменения. Нужно выяснить, что обвинение обещает воле, неспособной отменить прошлое.",
          "en": "Follow how powerlessness before time becomes revenge in Zarathustra’s account and then explains existence through punishment. Responsibility’s question here begins from imputation’s origin. Ask what accusation promises a will unable to undo the past."
        },
        "question": {
          "ru": "Каким ответом прошлому становится наказание в этом рассуждении?",
          "en": "What response to the past does punishment become in this argument?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be","tos.text-unit.sid-a602dadebebc6eb316a0caa49c4ca781"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:creation",
        "title": {"ru":"Творящий ответ прошлому","en":"A creative response to the past"},
        "body": {
          "ru": "Финал маршрута остаётся у этой задачи: понять, как созидание меняет ответ бывшему, не делая бывшее несовершившимся.",
          "en": "The route ends at this task: understanding how creation changes the response to what was without undoing it.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-ff240a868709abef0da385e34299d2c3"],"s":0},
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-339e330ca6a6a19079d4f5a3194cb1f7","tos.text-unit.sid-351ef082d5a4c98dcd346a2cbfc1357f"]}
          ]
        },
        "question": {
          "ru": "Что должно измениться в самой воле, чтобы её ответ прошлому стал творящим?",
          "en": "What must change in willing itself for its response to the past to become creative?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-351ef082d5a4c98dcd346a2cbfc1357f"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "demo:fate",
        "to": "demo:camus",
        "edgeId": "atlas:camus:interprets:fate",
        "body": {
          "ru": "От двух значений «моей судьбы» переходим к тому, как Камю соединяет поступки.",
          "en": "From two meanings of “my fate,” turn to how Camus joins actions."
        }
      },
      {
        "from": "demo:camus",
        "to": "demo:sisyphus",
        "edgeId": "atlas:camus:interprets:sisyphus",
        "body": {
          "ru": "Формулировку о судьбе возвращаем в сцену спуска, где она получает смысл.",
          "en": "Return the formulation about fate to the descent where it gains its sense."
        }
      },
      {
        "from": "demo:sisyphus",
        "to": "demo:agency",
        "edgeId": "atlas:sisyphus:develops:agency",
        "body": {
          "ru": "Ясный спуск позволяет спросить, как человек участвует в повторяемом усилии.",
          "en": "The lucid descent asks how a person participates in repeated effort."
        }
      },
      {
        "from": "demo:agency",
        "to": "demo:decision",
        "edgeId": "atlas:agency:develops:decision",
        "body": {"cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
      },
      {
        "from": "demo:decision",
        "to": "demo:counterfactual",
        "edgeId": "atlas:decision:develops:counterfactual",
        "body": {
          "ru": "Требование того же ведёт к воле, которая не может хотеть назад.",
          "en": "The demand for the same leads to willing’s inability to will backward."
        }
      },
      {
        "from": "demo:counterfactual",
        "to": "demo:responsibility",
        "edgeId": "atlas:counterfactual:develops:responsibility",
        "body": {
          "ru": "Бессилие перед прошлым вводит происхождение вменения и наказания.",
          "en": "Powerlessness before the past introduces the origin of imputation and punishment."
        }
      },
      {
        "from": "demo:responsibility",
        "to": "demo:creation",
        "edgeId": "atlas:responsibility:develops:creation",
        "body": {
          "ru": "От мести времени переходим к вопросу о творящем ответе бывшему.",
          "en": "From revenge against time, move toward a creative response to what was."
        }
      }
    ],
    "conclusion": {
      "ru": "Создание судьбы получает несколько точных смыслов: поступки совершаются, память соединяет их, воля спрашивает о своём отношении к уже бывшему. Камю и «Об избавлении» по-разному связывают эти движения с продолжающейся жизнью. Следующее чтение можно строить вокруг оставшегося вопроса: как творящее отношение к прошлому связано с хотением будущего?",
      "en": "Making fate acquires several precise meanings: actions are performed, memory joins them, and willing questions its relation to what has been. Camus and “Redemption” connect these movements with continuing life differently. A further reading can follow the remaining question: how does a creative relation to the past connect with willing the future?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b","tos.text-unit.sid-ff240a868709abef0da385e34299d2c3"],"s":106},
        {"w":"le-mythe-de-sisyphe","loc":"subtle"}
      ]
    },
    "investigation": {
      "startingPoint": {
        "ru": "Поставьте рядом «воля хочет такой судьбы» и судьбу, собранную из поступков взглядом памяти. Пусть различие этих формулировок задаст вопрос о слове «создавать».",
        "en": "Place the will wanting such a fate beside a fate gathered from actions by memory’s gaze. Let their difference pose the question of what making means."
      },
      "stakes": {
        "ru": "Связь действия с судьбой меняется вместе с направлением взгляда: вперёд к хотению, назад к сделанному, к самому ходу повторяемого труда. Отсюда возникает трудность ответа уже бывшему.",
        "en": "Action’s relation to fate changes with the gaze’s direction: forward toward willing, backward toward what was done, toward repeated labour’s course. Answering what has been becomes the difficulty."
      },
      "carryForward": {
        "ru": "Сохраните различие между поступком, связью памяти и творящим отношением к прошлому. При следующем чтении «Об избавлении» проверьте, как каждое из них участвует в словах «я так хотел».",
        "en": "Keep the distinction between an act, memory’s connection and a creative relation to the past. On rereading “Redemption,” examine how each enters “I willed it thus”."
      }
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"],"s":93,"e":135}
        ]
      },
      {
        "ref": "camus-sisyphus",
        "cite": [
          {"w":"le-mythe-de-sisyphe","loc":"return"},
          {"w":"le-mythe-de-sisyphe","loc":"tragic"},
          {"w":"le-mythe-de-sisyphe","loc":"subtle"},
          {"w":"le-mythe-de-sisyphe","loc":"joy"}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"],"s":0,"e":34},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b","tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b"],"s":106,"e":127}
        ]
      }
    ]
  },
  {
    "id": "moment-time",
    "title": {"ru":"Мгновение и время","en":"The moment and time"},
    "description": {
      "ru": "Пройдите от надписи «Мгновение» к двум вечным дорогам и вопросам об их связанности. Затем сравните необходимость в этой сцене с невозможностью хотеть назад и с определением свободы у Спинозы; завершите у поправки Заратустры о воле и судьбе.",
      "en": "Move from “Moment” to the two eternal paths and questions of their connection. Then compare necessity in this scene with inability to will backward and Spinoza’s definition of freedom; finish at Zarathustra’s correction about will and fate."
    },
    "question": {
      "ru": "Как вопрос о связанности времён превращается в вопрос о свободе воли?",
      "en": "How does a question about connected times become one about willing’s freedom?"
    },
    "steps": [
      {
        "nodeId": "moment",
        "title": {"ru":"Прочитать надпись","en":"Read the inscription"},
        "body": {
          "ru": "Прежде всякого ответа о времени глава помещает настоящее в пространственную сцену. Прочитайте описание до ответа карлика, сохранив то, о чём Заратустра ещё только спрашивает.",
          "en": "Before any answer about time, the chapter places the present in a spatial scene. Read the description before the dwarf’s reply, preserving what Zarathustra has yet only asked.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-38b6718fc95c36a467127712c215fff0"],"s":49,"e":71},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"s":138,"e":197}
          ]
        },
        "question": {"ru":"Почему настоящее получает два лица?","en":"Why does the present receive two faces?"},
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
        "nodeId": "demo:two-paths",
        "title": {"ru":"Мысленно продолжить дороги","en":"Follow the paths in thought"},
        "body": {
          "ru": "Следуйте этому вопросу как переходу от увиденного расположения к мысленно продолжаемому пути.",
          "en": "Follow the question as a move from the arrangement seen to a path continued in thought.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-7b662f8453b880f152323256b23c2409","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]}
          ]
        },
        "question": {
          "ru": "Что изменилось между встречей дорог и предположением об их дальнейшем ходе?",
          "en": "What changes between the paths’ meeting and imagining their further course?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:temporal-knot",
        "title": {"ru":"Уточнить связанность","en":"Specify connectedness"},
        "body": {
          "ru": "Образ узла обозначает эту новую трудность: выяснить, как из встречи направлений возникло требование связи всего.",
          "en": "The knot marks this new difficulty: how did meeting directions lead to the demand that everything be connected?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"],"s":52},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"],"s":0,"e":118}
          ]
        },
        "question": {
          "ru": "Что связывает мгновение с тем, что ещё только должно случиться?",
          "en": "What binds the moment to what has yet to happen?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:necessity",
        "title": {"ru":"Два хода необходимости","en":"Two movements of necessity"},
        "body": {
          "ru": "Сопоставьте необходимость повторения с неподвластностью совершившегося; каждый ход требует собственного объяснения.",
          "en": "Compare recurrence’s necessity with the accomplished past’s resistance; each movement calls for its own account.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-aa80ebe6fb9de20b613cb4f53ed7e855"]},
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"],"s":0,"e":34}
          ]
        },
        "question": {
          "ru": "Какая связь нужна, чтобы от неподвластного прошлого перейти к необходимому повтору?",
          "en": "What connection is needed to move from an unalterable past to necessary recurrence?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
            ]
          },
          {
            "ref": "z-redemption",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51"],"s":1,"e":70}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:free-necessary",
        "title": {"ru":"Необходимость собственной природы","en":"One’s own nature’s necessity"},
        "body": {
          "ru": "Этот вход меняет сам вопрос: необходимо выяснить источник определения, прежде чем называть всякую необходимость несвободой.",
          "en": "This entry changes the question itself: determine the source before calling every necessity unfreedom.",
          "cite": [{"w":"ethica","loc":"I.D7"}]
        },
        "question": {
          "ru": "Что означает определять себя к действию при сохранении необходимости?",
          "en": "What does determining oneself to act mean while necessity is retained?"
        },
        "grounds": [{"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}]
      },
      {
        "nodeId": "demo:fate",
        "title": {"ru":"Хотеть такой судьбы","en":"Wanting such a fate"},
        "body": {
          "ru": "Вернитесь с поправкой Заратустры к свободе и необходимости: отношение к судьбе теперь сформулировано как желание становления.",
          "en": "Bring Zarathustra’s correction back to freedom and necessity: the relation to fate is now formulated as desiring becoming.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]},
            {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"],"s":0}
          ]
        },
        "question": {
          "ru": "Почему хотение такой судьбы требует поправить первое определение воли?",
          "en": "Why does wanting such a fate require correcting the first account of willing?"
        },
        "grounds": [
          {
            "ref": "z-happy-isles",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-d9b7662287eb462c792ecb8350732485","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "moment",
        "to": "demo:two-paths",
        "edgeId": "atlas:moment:supports:two-paths",
        "body": {
          "ru": "Надпись ведёт к дорогам, чья встреча даёт мгновению два лица.",
          "en": "The inscription leads to the paths whose meeting gives the moment two faces."
        }
      },
      {
        "from": "demo:two-paths",
        "to": "demo:temporal-knot",
        "edgeId": "atlas:two-paths:supports:temporal-knot",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]}
          ]
        }
      },
      {
        "from": "demo:temporal-knot",
        "to": "demo:necessity",
        "edgeId": "atlas:temporal-knot:develops:necessity",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
          ]
        }
      },
      {
        "from": "demo:necessity",
        "to": "demo:free-necessary",
        "edgeId": "atlas:necessity:develops:free-necessary",
        "body": {
          "ru": "Для вопроса о свободе сравниваем необходимость с источником определения к действию.",
          "en": "For freedom’s question, compare necessity with determination to act’s source."
        }
      },
      {
        "from": "demo:free-necessary",
        "to": "demo:fate",
        "edgeId": "atlas:free-necessary:questions:fate",
        "body": {
          "ru": "Различение определения возвращаем к словам Заратустры о хотении судьбы.",
          "en": "Bring the distinction of determination back to Zarathustra’s words about wanting fate."
        }
      }
    ],
    "conclusion": {
      "ru": "Сцена ворот ставит необходимость через вечные пути и связанность мгновения; «Об избавлении» — через волю перед уже бывшим; Спиноза различает определение собственной природой и другим. Эти ходы позволяют точнее прочитать поправку о судьбе: что воля хочет, когда хочет становления? Продолжение вопроса лежит в связи этого желания с творчеством на «Блаженных островах».",
      "en": "The gateway scene poses necessity through eternal paths and the moment’s connection; “Redemption” through willing before what has been; Spinoza distinguishes determination by one’s own nature and another. These movements sharpen the correction about fate: what does willing want when it wants becoming? Continue through this desire’s relation to creation on “The Happy Isles”."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Начните до ответа карлика и сохраните вопрос Заратустры: останется ли противоречие дорог вечным? Дальнейшие шаги будут уточнять, как из него возникает необходимость возвращения.",
        "en": "Begin before the dwarf’s answer and keep Zarathustra’s question: will the paths’ opposition last eternally? Further steps specify how recurrence’s necessity arises from it."
      },
      "stakes": {
        "ru": "Одно слово «необходимо» участвует здесь в разных рассуждениях. Их различие меняет сам предмет вопроса о воле: ход времени, уже совершённое или источник действия.",
        "en": "The word necessary enters different arguments here. Their difference changes willing’s question: time’s course, what has been accomplished or action’s source."
      },
      "carryForward": {
        "ru": "Выберите один переход в цепочке вопросов у ворот и назовите его предпосылку. Затем сопоставьте её с тем, что сохраняется и меняется в словах «моя воля хочет такой судьбы».",
        "en": "Choose one transition in the gateway questions and name its premise. Then compare it with what is retained and changed in “my will wants such a fate”."
      }
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      },
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"],"s":0,"e":34}
        ]
      },
      {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]},
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      }
    ]
  },
  {
    "id": "repetition-difference",
    "title": {"ru":"Повтор и различие","en":"Repetition and difference"},
    "description": {
      "ru": "Начните с настойчивого исключения новой и лучшей жизни в речи зверей. Затем прочитайте, почему Делёз считает механический цикл недостаточным объяснением, и вернитесь к ребёнку из «Трёх превращений»: как новое начало оказывается рядом с образом самокатящегося колеса?",
      "en": "Begin with the animals’ insistent exclusion of a new or better life. Then read why Deleuze finds the mechanical cycle insufficient as an explanation, and return to the child of “The Three Metamorphoses”: how does a new beginning stand beside the self-rolling wheel?"
    },
    "question": {
      "ru": "Как совместно мыслить ту же жизнь, различие и новое начало?",
      "en": "How can the same life, difference and a new beginning be thought together?"
    },
    "steps": [
      {
        "nodeId": "same-life",
        "title": {"ru":"Ни новая, ни лучшая","en":"Neither new nor better"},
        "body": {
          "ru": "Сохраните всю силу этих исключений как исходную трудность маршрута.",
          "en": "Preserve these exclusions’ full force as the route’s initial difficulty.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"],"s":0},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"],"s":93,"e":160}
          ]
        },
        "question": {
          "ru": "Что потеряется из мысли о возвращении, если оставить только сходство?",
          "en": "What would recurrence lose if only resemblance were retained?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:identity",
        "title": {"ru":"Тот же ход подробностей","en":"The same course of particulars"},
        "body": {
          "ru": "Теперь вопрос к последующему прочтению точен: какую роль оно даст требованию того же порядка?",
          "en": "The question for the next reading is now precise: what role will it give the demand for the same order?",
          "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
        },
        "question": {
          "ru": "Почему сохранения главных событий недостаточно для этой формулы?",
          "en": "Why is retaining the main events insufficient for this formula?"
        },
        "grounds": [
          {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-ce045055585467cfc09217fef32bfa81","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"],"s":24,"e":96}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:return-difference",
        "title": {"ru":"Что не объясняет цикл?","en":"What does a cycle leave unexplained?"},
        "body": {"cite":[{"w":"nietzsche-et-la-philosophie","loc":"II.5"}]},
        "question": {
          "ru": "Что следует объяснить помимо совпадения начального и конечного состояния?",
          "en": "What needs explaining beyond matching initial and final states?"
        },
        "grounds": [{"ref":"deleuze-nietzsche-return","cite":[{"w":"nietzsche-et-la-philosophie","loc":"II.5"}]}]
      },
      {
        "nodeId": "demo:difference",
        "title": {"ru":"Различающее основание","en":"A differential ground"},
        "body": {
          "ru": "Удержите различие между предложенным основанием и ещё требуемым объяснением.",
          "en": "Retain the distinction between a proposed ground and the explanation still required.",
          "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
        },
        "question": {
          "ru": "Что именно должно быть воспроизведено, если основание само различающее?",
          "en": "What must be reproduced if the ground is itself differential?"
        },
        "grounds": [{"ref":"deleuze-nietzsche-return","cite":[{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]}]
      },
      {
        "nodeId": "demo:same-new",
        "title": {"ru":"Сделать новым само повторение","en":"Make repetition itself new"},
        "body": {
          "ru": "Прочитайте это место рядом с требованием той же жизни: новизна теперь относится и к тому, как повторение хотят.",
          "en": "Read this passage beside the same-life demand: novelty now concerns how repetition is willed as well.",
          "cite": [{"w":"difference-et-repetition","loc":"intro.programme"}]
        },
        "question": {
          "ru": "Как изменяется вопрос о новом, если повторение становится предметом желания?",
          "en": "How does novelty’s question change when repetition becomes desire’s object?"
        },
        "grounds": [
          {"ref":"deleuze-repetition","cite":[{"w":"difference-et-repetition","loc":"intro.programme"}]},
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"],"s":93}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:creation",
        "title": {"ru":"Ребёнок и самокатящееся колесо","en":"The child and the self-rolling wheel"},
        "body": {
          "ru": "Вернитесь к исходному вопросу через это соседство: новизна, забывание и собственное движение уже соединены в одном образе.",
          "en": "Return to the initial question through this conjunction: novelty, forgetting and self-movement already meet in one image.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1444d006a038716b06a33646bf0b0439"]},
            {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
          ]
        },
        "question": {
          "ru": "Что образ ребёнка позволяет спросить о возвращении, чего не давала одна геометрия круга?",
          "en": "What does the child’s image let us ask about recurrence beyond a circle’s geometry?"
        },
        "grounds": [
          {
            "ref": "z-metamorphoses",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
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
          "ru": "Исключения зверей требуют уточнить, что именно возвращается тем же.",
          "en": "The animals’ exclusions require specifying what returns as the same."
        }
      },
      {
        "from": "demo:identity",
        "to": "demo:return-difference",
        "edgeId": "atlas:return-difference:questions:identity",
        "body": {
          "ru": "К строгому тождеству обращаем делёзовский вопрос об объяснении самого движения.",
          "en": "Address Deleuze’s question of movement’s explanation to this strict identity."
        }
      },
      {
        "from": "demo:return-difference",
        "to": "demo:difference",
        "edgeId": "atlas:return-difference:interprets:difference",
        "body": {"cite":[{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]}
      },
      {
        "from": "demo:difference",
        "to": "demo:same-new",
        "edgeId": "atlas:same-new:questions:difference",
        "body": {
          "ru": "От различия как основания переходим к повторению как предмету воли.",
          "en": "From difference as ground, turn to repetition as willing’s object."
        }
      },
      {
        "from": "demo:same-new",
        "to": "demo:creation",
        "edgeId": "atlas:creation:develops:same-new",
        "body": {
          "ru": "Вопрос о новом возвращаем в образ ребёнка, который начинает и катится сам.",
          "en": "Return novelty’s question to the child who begins and rolls by itself."
        }
      }
    ],
    "conclusion": {
      "ru": "Маршрут различил строгость повторяемой жизни, объяснение движения и новизну самого хотения повтора. Дальнейшее чтение должно показать, как эти ходы объясняют друг друга и где требуют разных значений «того же».",
      "en": "The route distinguishes the recurring life’s strictness, movement’s explanation and novelty in willing recurrence itself. Further reading must show how these movements illuminate one another and where they require different meanings of the same.",
      "cite": [
        {"w":"nietzsche-et-la-philosophie","loc":"II.5"},
        {"w":"nietzsche-et-la-philosophie","loc":"II.6"},
        {"w":"difference-et-repetition","loc":"intro.programme"},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"],"s":37,"e":93}
      ]
    },
    "investigation": {
      "startingPoint": {
        "ru": "Сохраните формулу зверей целиком: не новая, не лучшая и не подобная жизнь. Все дальнейшие различения возвращайте к этим трём исключениям.",
        "en": "Keep the animals’ formula whole: neither a new, better nor similar life. Bring every further distinction back to these exclusions."
      },
      "stakes": {
        "ru": "От роли тождества зависит ход объяснения: дан ли повтор заранее или его ещё нужно получить из различающего отношения? Отсюда меняется и вопрос о новизне созидания.",
        "en": "Identity’s role determines the explanation’s course: is recurrence given in advance, or must it still be obtained from a differential relation? Creation’s novelty changes with that question."
      },
      "carryForward": {
        "ru": "Сопоставьте две фразы: «та же жизнь» и «самокатящееся колесо». Укажите, что для их связи объясняет делёзовское чтение и какую задачу оставляет следующему обращению к тексту.",
        "en": "Compare “the same life” and “a self-rolling wheel.” State what Deleuze’s reading explains about their connection and what task it leaves for returning to the text."
      }
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"],"s":93}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "deleuze-nietzsche-return",
        "cite": [{"w":"nietzsche-et-la-philosophie","loc":"II.5"},{"w":"nietzsche-et-la-philosophie","loc":"II.6"}]
      },
      {"ref":"deleuze-repetition","cite":[{"w":"difference-et-repetition","loc":"intro.programme"}]},
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"],"s":37,"e":93}
        ]
      }
    ]
  },
  {
    "id": "affirmation-pain-action",
    "title": {"ru":"Утверждение, боль и действие","en":"Affirmation, pain and action"},
    "description": {
      "ru": "От желания научиться amor fati перейдите к радости, которая хочет вечности всего. Затем прочитайте «О сострадательных» и «Об избавлении»: конкретная встреча с другом и ответ воли прошлому уточняют вопрос о том, как утверждение входит в поступок.",
      "en": "Move from wanting to learn amor fati to joy desiring everything’s eternity. Then read “The Pitiful” and “Redemption”: an encounter with a friend and willing’s response to the past specify how affirmation enters action."
    },
    "question": {
      "ru": "Как желание утверждать всё становится определённым отношением и поступком?",
      "en": "How does wanting to affirm everything become a definite relation and action?"
    },
    "steps": [
      {
        "nodeId": "demo:amor-fati",
        "title": {"ru":"Учиться amor fati","en":"Learning amor fati"},
        "body": {
          "ru": "Прочитайте последовательность §276 как движение ещё предстоящего учения.",
          "en": "Read the sequence of §276 as the movement of learning still ahead.",
          "cite": [{"w":"die-froehliche-wissenschaft","loc":"276"}]
        },
        "question": {
          "ru": "Как внутри этого желания соединяются видеть и делать прекрасным?",
          "en": "How are seeing and making beautiful joined within this wish?"
        },
        "grounds": [{"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}]
      },
      {
        "nodeId": "demo:affirmation",
        "title": {"ru":"От будущего «да» к его объёму","en":"From a future yes to its scope"},
        "body": {
          "ru": "Желание однажды стать утверждающим ещё оставляет вопрос, чего хочет это «да». Проследите, как личное желание расширяется в самой речи.",
          "en": "Wanting one day to become affirmative still leaves what this yes wants in question. Follow the expansion of personal desire within the speech itself.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-9a3da971437436d2ef97a5e5de89a4b2"],"s":0}
          ]
        },
        "question": {
          "ru": "Почему утверждённое мгновение не остаётся отдельным мгновением?",
          "en": "Why does an affirmed moment not remain an isolated moment?"
        },
        "grounds": [
          {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]},
          {
            "ref": "z-drunken-song",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:joy-pain",
        "title": {"ru":"Радость хочет себя и всего","en":"Joy wants itself and everything"},
        "body": {
          "ru": "Удержите обе ступени: различие хотений и затем расширение одного из них.",
          "en": "Keep both stages: the differing forms of willing and then one’s expansion.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-b72032847012285d32829661efad3ea3","tos.text-unit.sid-2a8aada5d848098eed8b2b160fafa871"],"s":0},
            {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
          ]
        },
        "question": {
          "ru": "Как песнь переходит от различия радости и горя к желанию всего?",
          "en": "How does the song move from joy’s difference from sorrow to wanting everything?"
        },
        "grounds": [
          {
            "ref": "z-drunken-song",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-a0d84a7b3f6d5093f616ea7159f64287","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:shared-world",
        "title": {"ru":"Встретить страдающего друга","en":"Encounter the suffering friend"},
        "body": {
          "ru": "Прочитайте это обращение рядом со всеобщим хотением радости и спросите, как мысль получает определённость в отношении.",
          "en": "Read this address beside joy’s universal willing and ask how thought becomes definite within a relation.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-e265be83aa60fd32629dc350aa3c9ebc"]},
            {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a","tos.text-unit.sid-a4476436db21aa889c21f26228aa6672"],"s":40}
          ]
        },
        "question": {
          "ru": "Что друг должен получить от любви, чего ещё не даёт одна жалость?",
          "en": "What must the friend receive from love beyond what pity alone gives?"
        },
        "grounds": [
          {
            "ref": "z-pitiful",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-e265be83aa60fd32629dc350aa3c9ebc"]},
              {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a","tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a"],"s":40,"e":119}
            ]
          },
          {
            "ref": "z-drunken-song",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:responsibility",
        "title": {"ru":"Ответ и действие дара","en":"Response and the gift’s action"},
        "body": {
          "ru": "Это заставляет исследовать сам способ отвечать другому. Затем в «Об избавлении» месть получает другое основание — бессилие воли перед временем; сопоставление уточняет, откуда возникает требование ответа.",
          "en": "This directs inquiry toward the manner of answering another. In “Redemption,” revenge receives another ground—willing’s powerlessness before time; comparison specifies where a demand for response arises.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-6d5d64df7654526f06be7538126618e8","tos.text-unit.sid-6d5d64df7654526f06be7538126618e8"],"s":0,"e":66},
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-bc8111269b7adf1e6f1a2c297ae8b00b","tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be"]}
          ]
        },
        "question": {
          "ru": "Как изменяется предмет ответа между тяжестью дара и тяжестью прошлого?",
          "en": "How does response’s object change between a gift’s burden and the past’s burden?"
        },
        "grounds": [
          {
            "ref": "z-pitiful",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-6d5d64df7654526f06be7538126618e8","tos.text-unit.sid-09b9bca89edae4834da231d217a4db61"]}
            ]
          },
          {
            "ref": "z-redemption",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:counterfactual",
        "title": {"ru":"Хотеть иначе перед бывшим","en":"Willing otherwise before what was"},
        "body": {
          "ru": "Прочитайте творящее «я так хотел» рядом с желанием §276 не обвинять даже обвинителей: что должно измениться в отношении воли к прошлому?",
          "en": "Read the creative “I willed it thus” beside §276’s wish not to accuse even accusers: what must change in willing’s relation to the past?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-4a143af767f17dd9d433e2a8df84cfc6"],"s":0},
            {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-339e330ca6a6a19079d4f5a3194cb1f7"],"s":0}
          ]
        },
        "question": {
          "ru": "Какое хотение остаётся возможным перед тем, что уже совершилось?",
          "en": "What willing remains possible before what has already happened?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"],"s":0,"e":34},
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-bc8111269b7adf1e6f1a2c297ae8b00b","tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be"]},
              {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b","tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b"],"s":106,"e":127}
            ]
          },
          {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]}
        ]
      },
      {
        "nodeId": "demo:decision",
        "title": {"ru":"Вопрос при каждом действии","en":"A question in every action"},
        "body": {
          "ru": "После чтения о радости, помощи и мести этот вопрос получает определённый материал: как отношение к происходящему входит в то, что человек совершает?",
          "en": "After joy, help and revenge, the question has definite material: how does a relation to events enter what a person does?",
          "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
        },
        "question": {
          "ru": "Что значит хотеть повторения именно этого способа действовать?",
          "en": "What does it mean to want this very way of acting to return?"
        },
        "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}]
      }
    ],
    "transitions": [
      {
        "from": "demo:amor-fati",
        "to": "demo:affirmation",
        "edgeId": "atlas:amor-fati:develops:affirmation",
        "body": {
          "ru": "Учение amor fati открывает вопрос об объёме будущего утверждения.",
          "en": "Learning amor fati opens the scope of a future affirmation."
        }
      },
      {
        "from": "demo:affirmation",
        "to": "demo:joy-pain",
        "edgeId": "atlas:affirmation:develops:joy-pain",
        "body": {
          "ru": "Одно утверждённое мгновение ведёт к различным хотениям радости и горя.",
          "en": "One affirmed moment leads to joy’s and sorrow’s different willing."
        }
      },
      {
        "from": "demo:joy-pain",
        "to": "demo:shared-world",
        "edgeId": "atlas:joy-pain:develops:shared-world",
        "body": {
          "ru": "От всеобщего желания переходим к конкретному обращению к страдающему другу.",
          "en": "From universal desire, turn to addressing a suffering friend."
        }
      },
      {
        "from": "demo:shared-world",
        "to": "demo:responsibility",
        "edgeId": "atlas:shared-world:develops:responsibility",
        "body": {
          "ru": "Неоднозначность помощи ставит вопрос о собственном способе отвечать другому.",
          "en": "Help’s ambiguity asks how one answers another."
        }
      },
      {
        "from": "demo:responsibility",
        "to": "demo:counterfactual",
        "edgeId": "atlas:counterfactual:develops:responsibility",
        "body": {
          "ru": "Через тему мести возвращаемся к воле, бессильной изменить бывшее.",
          "en": "Through revenge, return to willing’s inability to alter what has been."
        }
      },
      {
        "from": "demo:counterfactual",
        "to": "demo:decision",
        "edgeId": "atlas:decision:develops:counterfactual",
        "body": {
          "ru": "От ответа прошлому переходим к вопросу повторения при нынешнем действии.",
          "en": "From answering the past, turn to recurrence’s question in present action."
        }
      }
    ],
    "conclusion": {
      "ru": "Учение взгляда в §276, желание вечности в «Песни опьянения», действие дара и месть времени оказались разными местами одного продолжаемого вопроса об утверждении. Связь с поступком проходит через то, как человек видит, хочет и отвечает. Возвращение к §341 позволяет продолжить чтение: какое превращение должно сделать желание повторения собственным вопросом при каждом действии?",
      "en": "Learning to see in §276, desire for eternity in “The Drunken Song,” the gift’s action and revenge against time become different places of an ongoing question about affirmation. The connection to action passes through how a person sees, wills and responds. Returning to §341 continues the reading: what transformation would make desiring recurrence one’s own question in every action?"
    },
    "investigation": {
      "startingPoint": {
        "ru": "Начните с будущего времени §276: говорящий ещё хочет научиться видеть и стать утверждающим. Пусть вопрос о том, чему именно предстоит учиться, ведёт к следующим местам.",
        "en": "Begin with §276’s future: the speaker still wants to learn to see and become affirmative. Let what must be learned lead into the following passages."
      },
      "stakes": {
        "ru": "Желание всего должно получить связь с определённым обращением, даром и отношением к прошлому. Эти тексты дают собственные трудности такого перехода: стыд помощи, тяжесть обязательства и месть времени.",
        "en": "Wanting everything must connect with a definite address, gift and relation to the past. These texts supply the transition’s own difficulties: help’s shame, obligation’s burden and revenge against time."
      },
      "carryForward": {
        "ru": "Выберите одну связь между прочитанными местами — например, «не обвинять» и ответ бывшему. Вернитесь с ней к вопросу §341 о каждом действии и уточните, какого превращения он требует.",
        "en": "Choose one connection between the passages—for example, not accusing and answering what was. Bring it back to §341’s question about every action and specify the transformation it requires."
      }
    },
    "grounds": [
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]},
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-a0d84a7b3f6d5093f616ea7159f64287","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
        ]
      },
      {
        "ref": "z-pitiful",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-b66e16ad7e2a0418f629b219183c96b8","tos.text-unit.sid-09b9bca89edae4834da231d217a4db61"]},
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-40f2feef59d2e64bb98d3eb59bf8f81a","tos.text-unit.sid-a4476436db21aa889c21f26228aa6672"],"s":40}
        ]
      },
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51"],"s":72,"e":108},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-bc8111269b7adf1e6f1a2c297ae8b00b","tos.text-unit.sid-d0da4f88d01d03d4f85805f9f62a77be"]},
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b","tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b"],"s":106,"e":127}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ]
  },
  {
    "id": "reading-voices",
    "title": {"ru":"Чтение и голоса","en":"Reading and voices"},
    "description": {
      "ru": "Прочитайте песню зверей вместе с тем, что ей предшествует и следует за ней: радость речи, упрёк шарманке, воспоминание о болезни и совет сделать новую лиру. Маршрут проследит, как эти голоса доходят до задачи учить возвращению и до собственного дела Заратустры.",
      "en": "Read the animals’ song with what precedes and follows it: speech’s joy, the barrel-organ reproach, remembered illness and the advice to make a new lyre. Follow these voices toward the task of teaching recurrence and Zarathustra’s own work."
    },
    "question": {
      "ru": "Как содержание возвращения меняется вместе со способом его произнести и услышать?",
      "en": "How does recurrence’s content change with the way it is voiced and heard?"
    },
    "steps": [
      {
        "nodeId": "all-things",
        "title": {"ru":"Все вещи — для кого?","en":"All things—for whom?"},
        "body": {
          "ru": "Начните с этой рамки: всеобщая речь введена через определённый способ думать.",
          "en": "Begin with this frame: universal speech is introduced through a particular way of thinking.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c"],"s":45,"e":98},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
          ]
        },
        "question": {
          "ru": "Что добавляет к словам о всех вещах оговорка «для тех, кто думает, как мы»?",
          "en": "What does “for those who think as we do” add to the words about all things?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:animals",
        "title": {"ru":"Когда звери начинают говорить","en":"When the animals begin to speak"},
        "body": {
          "ru": "Забота и речь входят в выздоровление ещё до изложения учения.",
          "en": "Care and speech enter recovery before the teaching is expounded.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-524ba7995cb645de150e842f8518f37e","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-33bda0fcdf4170fb11f4dee205767dcd","tos.text-unit.sid-33bda0fcdf4170fb11f4dee205767dcd"],"s":0,"e":66},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4","tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4"],"s":128,"e":190}
          ]
        },
        "question": {
          "ru": "Почему приятный запах и приглашение слушать важны для последующей формулы?",
          "en": "Why do the pleasant scent and invitation to listen matter for the formula that follows?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-524ba7995cb645de150e842f8518f37e","tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:voice-reading",
        "title": {"ru":"Кто танцует на вещах?","en":"Who dances upon things?"},
        "body": {
          "ru": "Различие голосов касается здесь отношения языка к миру, а не только подписи под формулой.",
          "en": "The voices differ over language’s relation to the world, beyond who signs the formula.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c24afcda817ec99be27398e1eeaa30b5","tos.text-unit.sid-bd057f6afaca60ed7da9b4ed85f0d640"],"s":50},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c8e9e850b816f1db51f391e81ec26f51","tos.text-unit.sid-c8e9e850b816f1db51f391e81ec26f51"],"s":93,"e":175},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c"],"s":74,"e":174}
          ]
        },
        "question": {
          "ru": "Как меняется утверждение при переходе от танцующего говорящего к танцующим вещам?",
          "en": "How does the claim change from a dancing speaker to dancing things?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c24afcda817ec99be27398e1eeaa30b5","tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:source-neighbor",
        "title": {"ru":"Услышать следующий ответ","en":"Hear the following reply"},
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da"],"s":2,"e":82},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-c1a61ec1014026941c54254a0978115d"],"s":66,"e":120},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4f156d0bad165d016f941204caf55153","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
          ]
        },
        "question": {
          "ru": "Что позволяет улыбке и упрёку стоять в одной реплике?",
          "en": "What allows the smile and reproach to inhabit one reply?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:narrated-voices",
        "title": {"ru":"Речь прерывается советом петь","en":"Speech interrupted by advice to sing"},
        "body": {
          "ru": "Теперь карта голосов показывает, как разговор сам пытается изменить состояние говорящего.",
          "en": "The voices now show the conversation itself trying to change the speaker’s condition.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-9f1471144e729291311298ed75a2a53b","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"],"s":55,"e":173},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-1fdb6b338b3ca379b3fc0d4b9a295d84","tos.text-unit.sid-1fdb6b338b3ca379b3fc0d4b9a295d84"],"s":1,"e":35},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-4b6e322412bf32bc28e4e88b067989cd"],"s":3}
          ]
        },
        "question": {
          "ru": "Почему ответом на рассказ о болезни становится смена способа речи?",
          "en": "Why does changing the manner of speech answer an illness account?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-9f1471144e729291311298ed75a2a53b","tos.text-unit.sid-4b6e322412bf32bc28e4e88b067989cd"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:character-author",
        "title": {"ru":"Речь внутри речи","en":"Speech within speech"},
        "body": {
          "ru": "Прочтению предстоит объяснить всю рамку.",
          "en": "A reading must explain the entire frame.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c"],"s":87,"e":128},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"],"s":0},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"],"s":0,"e":110},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
          ]
        },
        "question": {
          "ru": "Как вложенная речь и заключительное молчание влияют на её авторскую атрибуцию?",
          "en": "How do the embedded speech and closing silence affect authorial attribution?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:task",
        "title": {"ru":"Названная судьба учителя","en":"The teacher’s named fate"},
        "body": {
          "ru": "Сопоставьте задачу, названную другими, с собственным наступившим часом.",
          "en": "Compare a task named by others with one’s own arriving hour.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e","tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e"],"s":39,"e":116},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"],"s":50,"e":134},
            {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-550efec95e443c4bd2b7c2897f5a45c2","tos.text-unit.sid-550efec95e443c4bd2b7c2897f5a45c2"],"s":1,"e":52},
            {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae","tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae"],"s":78,"e":107}
          ]
        },
        "question": {
          "ru": "Что нужно прочитать между называнием предназначения и словами о собственном деле?",
          "en": "What needs to be read between naming a vocation and speaking of one’s own work?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
            ]
          },
          {
            "ref": "z-sign",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-c0e3a3517b021107cee042466f68a20d","tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:teach-live",
        "title": {"ru":"Нести мысль и выйти","en":"Bear the thought and depart"},
        "body": {
          "ru": "Сравнение делает учительство движением между состоянием говорящего, способом речи и готовностью к делу. Вернитесь к первоначальной песне зверей и спросите, что она уже совершает для этого движения.",
          "en": "Comparison makes teaching a movement between the speaker’s condition, speech’s form and readiness for work. Return to the animals’ first song and ask what it already does for that movement.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e","tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e"],"s":39,"e":116},
            {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-7ccacf7def856540e0472211fc54d3e2","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
          ]
        },
        "question": {
          "ru": "Что значит передавать мысль, которая ещё изменяет самого учителя?",
          "en": "What does it mean to convey a thought still changing the teacher?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-1d18f623343990179ed1f4ab76c81bfd","tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e"]}
            ]
          },
          {
            "ref": "z-sign",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-7ccacf7def856540e0472211fc54d3e2","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
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
          "ru": "Песню возвращаем к моменту, когда звери решают заговорить с выздоравливающим.",
          "en": "Return the song to the moment when the animals decide to address the convalescent."
        }
      },
      {
        "from": "demo:animals",
        "to": "demo:voice-reading",
        "edgeId": "atlas:voice-reading:interprets:animals",
        "body": {
          "ru": "Забота через речь ведёт к различию между словами о вещах и самими вещами.",
          "en": "Care through speech leads to the difference between words about things and things themselves."
        }
      },
      {
        "from": "demo:voice-reading",
        "to": "demo:source-neighbor",
        "edgeId": "atlas:source-neighbor:supports:voice-reading",
        "body": {
          "ru": "Ответ зверей читаем вместе с непосредственно следующим упрёком героя.",
          "en": "Read the animals’ reply with the protagonist’s immediate reproach."
        }
      },
      {
        "from": "demo:source-neighbor",
        "to": "demo:narrated-voices",
        "edgeId": "atlas:narrated-voices:develops:source-neighbor",
        "body": {
          "ru": "Соседние реплики разворачиваем в последовательность прерываний и смены речи.",
          "en": "Follow neighbouring replies into the sequence of interruptions and changing speech."
        }
      },
      {
        "from": "demo:narrated-voices",
        "to": "demo:character-author",
        "edgeId": "atlas:narrated-voices:develops:character-author",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
          ]
        }
      },
      {
        "from": "demo:character-author",
        "to": "demo:task",
        "edgeId": "atlas:character-author:questions:task",
        "body": {
          "ru": "Вложенная речь возвращает вопрос к предназначению учителя.",
          "en": "The embedded speech returns the question to the teacher’s vocation."
        }
      },
      {
        "from": "demo:task",
        "to": "demo:teach-live",
        "edgeId": "atlas:task:develops:teach-live",
        "body": {
          "ru": "Названную судьбу связываем с пением, выздоровлением и выходом к делу.",
          "en": "Connect the named fate with singing, recovery and departure toward work."
        }
      }
    ],
    "conclusion": {
      "ru": "Песня зверей участвует в заботе и объяснении, ответ Заратустры удерживает продолжающуюся болезнь, а новая лира соединяет речь с задачей учителя. Формула возвращения получила различимую композицию: кто говорит, как слышит другой и что разговор делает с ним. Следующее чтение может проверить, как в этой композиции связаны знание учения и способность его нести.",
      "en": "The animals’ song participates in care and explanation, Zarathustra’s reply retains continuing illness, and the new lyre connects speech with the teacher’s task. Recurrence’s formula now has a discernible composition: who speaks, how another hears and what the exchange does to them. Further reading can examine how knowing the teaching connects with being able to bear it."
    },
    "investigation": {
      "startingPoint": {
        "ru": "Начните с фразы зверей «для тех, кто думает, как мы». Затем прочитайте предшествующие слова Заратустры о призрачных мостах речи, чтобы услышать, на что звери отвечают.",
        "en": "Begin with the animals’ “for those who think as we do.” Then read Zarathustra’s preceding words about speech’s illusory bridges to hear what their answer addresses."
      },
      "stakes": {
        "ru": "Распределение голосов участвует в содержании: язык, мир, боль и учительство получают разные отношения в последовательных репликах. Атрибуция требует понять эту работу сцены.",
        "en": "The allocation of voices participates in the content: language, world, pain and teaching take different relations in successive replies. Attribution requires understanding the scene’s work."
      },
      "carryForward": {
        "ru": "Выберите один переход между репликами и сформулируйте, что он меняет в понимании возвращения. Проверьте эту формулировку по более поздней речи о новой лире и судьбе учителя.",
        "en": "Choose one transition between replies and state what it changes in understanding recurrence. Examine that statement through the later speech about the new lyre and the teacher’s fate."
      }
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      },
      {
        "ref": "z-sign",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
        ]
      }
    ]
  },
  {
    "id": "image-experience",
    "title": {"ru":"Образ и переживание","en":"Image and experience"},
    "description": {
      "ru": "Следуйте за видением пастуха от неудавшейся помощи к укусу и неслыханному смеху. Затем сравните желание рассказчика с вопросом §341 и с танцем «Семи печатей»; финал маршрута даст самому повторению припева изменить чтение.",
      "en": "Follow the shepherd’s vision from failed help to the bite and unprecedented laughter. Then compare the narrator’s desire with §341’s question and “The Seven Seals”’ dance; at the route’s end, let the refrain’s repetition itself change the reading."
    },
    "question": {
      "ru": "Как образ преображения становится желанием и способом произнести «да»?",
      "en": "How does an image of transformation become desire and a way of saying yes?"
    },
    "steps": [
      {
        "nodeId": "demo:shepherd",
        "title": {"ru":"Сначала увидеть удушье","en":"First see the choking"},
        "body": {
          "ru": "Прочитайте эту последовательность до всякого названия символа, чтобы сохранить задачу действия внутри видения.",
          "en": "Read this sequence before naming a symbol, preserving the task of action within the vision.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e"],"s":53}
          ]
        },
        "question": {
          "ru": "Что именно видит рассказчик прежде, чем пытается помочь?",
          "en": "What does the narrator see before he tries to help?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e9175aa652493debe3e712faff10a3df","tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:bite",
        "title": {"ru":"Рука и крик","en":"The hand and the cry"},
        "body": {
          "ru": "Собственный укус пастуха отвечает на этот сложный призыв, меняя распределение действия.",
          "en": "The shepherd’s own bite answers this complex exhortation and redistributes the act.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"],"s":48,"e":159}
          ]
        },
        "question": {
          "ru": "Почему действие пастуха нуждается в призыве, который не сводится к одному чувству?",
          "en": "Why does the shepherd’s act need an exhortation irreducible to one feeling?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:laughter",
        "title": {"ru":"Смех, которого ещё не слышали","en":"A laughter never heard before"},
        "body": {
          "ru": "Видение изменило и того, кто его рассказывает.",
          "en": "The vision has changed its narrator too.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":0,"e":90},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38407ce82a4703428377d06e4af5c240","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"],"s":17}
          ]
        },
        "question": {
          "ru": "Как желание рассказчика становится частью смысла преображения?",
          "en": "How does the narrator’s desire become part of transformation’s meaning?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:affirmation",
        "title": {"ru":"Хотеть снова услышанного","en":"Wanting what was heard again"},
        "body": {
          "ru": "Желание смеха можно поставить рядом с огромным мгновением §341, после которого слушающий ответил бы демону как богу. В обоих местах один опыт меняет вопрос об отношении к жизни. Сравнение требует уточнить, чего именно хотят вновь: услышанного смеха или всей жизни в прежнем порядке.",
          "en": "The desire for laughter can stand beside §341’s immense moment, after which the hearer might address the demon as a god. In both passages, one experience changes the relation to life. Comparison asks what is wanted again: the laughter heard or the entire life in its previous order.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]},
            {"w":"die-froehliche-wissenschaft","loc":"341"}
          ]
        },
        "question": {
          "ru": "Как от желания одного переживания перейти к желанию возвращения жизни?",
          "en": "How can desire for one experience move toward desiring a life’s recurrence?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38407ce82a4703428377d06e4af5c240","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
            ]
          },
          {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
        ]
      },
      {
        "nodeId": "demo:performative",
        "title": {"ru":"Как желание произносится","en":"How desire is voiced"},
        "body": {
          "ru": "Переход позволяет исследовать, как утверждение получает форму, в которой его можно снова произнести.",
          "en": "The transition examines how affirmation receives a form in which it can be voiced again.",
          "cite": [
            {"w":"die-froehliche-wissenschaft","loc":"341"},
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
          ]
        },
        "question": {
          "ru": "Что повтор припева добавляет к одному признанию любви к вечности?",
          "en": "What does repeating the refrain add to a single avowal of love for eternity?"
        },
        "grounds": [
          {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
          {
            "ref": "z-seven-seals",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:dance",
        "title": {"ru":"Тяжёлое делается лёгким","en":"Heaviness becomes light"},
        "body": {
          "ru": "Проследите, как образ движения меняет самый способ обращения к вечности.",
          "en": "Follow how movement’s image changes the manner of addressing eternity.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-fa02b841750993c61618111d272a18aa"],"s":0,"e":42},
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-187d71e13bc9881187035bebd261de0c","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"],"s":31,"e":100},
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-40d09c0747d9bac0cf78568ff49e8cd0","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"],"s":120}
          ]
        },
        "question": {
          "ru": "Почему танец продолжается в переходе к пению?",
          "en": "Why does dance continue through the move to singing?"
        },
        "grounds": [
          {
            "ref": "z-seven-seals",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-37ba7e4b8c29104decfaa04e123c0e24"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:lived-rhythm",
        "title": {"ru":"Тот же припев после танца","en":"The same refrain after the dance"},
        "body": {
          "ru": "Вернитесь к припеву после шестой печати и сравните его с первым появлением. Ритм текста позволяет услышать, как повторение удерживает желание и вместе с тем меняет его окружение.",
          "en": "Return to the refrain after the sixth seal and compare its first appearance. The text’s rhythm makes repetition audible as retaining desire while changing its surroundings.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]},
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
          ]
        },
        "question": {
          "ru": "Что в знакомом припеве слышится благодаря предшествующему танцу?",
          "en": "What becomes audible in the familiar refrain through the preceding dance?"
        },
        "grounds": [
          {
            "ref": "z-seven-seals",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]},
              {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:return-practice",
        "title": {"ru":"Вернуться с изменившимся слухом","en":"Return with changed hearing"},
        "body": {
          "ru": "Прочитайте выбранные две строфы и припевы ещё раз, удерживая вопрос о желании, а затем вернитесь к последним словам о смехе пастуха. Текст даёт два способа хотеть «снова»: повтором слов и жаждой неслыханного смеха. Уточните связь между ними по тому, что теперь стало слышно.",
          "en": "Read the chosen stanzas and refrains again with desire’s question in view, then return to the final words about the shepherd’s laughter. The text offers two ways of wanting again: repeating words and longing for unprecedented laughter. Specify their connection through what has now become audible."
        },
        "question": {
          "ru": "Какой вопрос о смехе изменился после чтения припева?",
          "en": "Which question about laughter changed after reading the refrain?"
        },
        "grounds": [
          {
            "ref": "z-seven-seals",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]},
              {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"]}
            ]
          },
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38407ce82a4703428377d06e4af5c240","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
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
          "ru": "От увиденного удушья следуем к неудавшейся помощи и крику.",
          "en": "From the choking seen, follow failed help and the cry."
        }
      },
      {
        "from": "demo:bite",
        "to": "demo:laughter",
        "edgeId": "atlas:bite:develops:laughter",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"s":2}
          ]
        }
      },
      {
        "from": "demo:laughter",
        "to": "demo:affirmation",
        "edgeId": "atlas:laughter:interprets:affirmation",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
          ]
        }
      },
      {
        "from": "demo:affirmation",
        "to": "demo:performative",
        "edgeId": "atlas:performative:develops:affirmation",
        "body": {
          "ru": "Желание повторения требует формы, в которой оно действует и произносится.",
          "en": "Desiring repetition calls for a form in which it acts and is voiced."
        }
      },
      {
        "from": "demo:performative",
        "to": "demo:dance",
        "edgeId": "atlas:dance:supports:performative",
        "body": {
          "ru": "В «Семи печатях» такую форму исследуем через танец и пение.",
          "en": "In “The Seven Seals,” examine such a form through dance and singing."
        }
      },
      {
        "from": "demo:dance",
        "to": "demo:lived-rhythm",
        "edgeId": "atlas:dance:develops:lived-rhythm",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-c00885a06bc25aa7eebc0d82c20fb581"],"s":0}
          ]
        }
      },
      {
        "from": "demo:lived-rhythm",
        "to": "demo:return-practice",
        "edgeId": "atlas:lived-rhythm:develops:return-practice",
        "body": {
          "ru": "Повтор припева позволяет вернуться к исходному желанию смеха с новым вопросом.",
          "en": "The refrain’s repetition lets us return to the initial longing for laughter with a new question."
        }
      }
    ],
    "conclusion": {
      "ru": "Видение связало помощь, собственный укус и желание рассказчика; песнь дала желанию возвращающуюся форму. Сравнение смеха и припева уточнило, как образ способен продолжать мысль через изменение слуха и ожидания. Дальнейшее чтение можно начать с этой связи: что значит желать вновь того, что однажды преобразило желание?",
      "en": "The vision connected help, the shepherd’s own bite and the narrator’s longing; the song gave desire a returning form. Comparing laughter with refrain sharpened how an image can carry thought through changed hearing and expectation. Further reading can begin here: what does it mean to want again what once transformed desire?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38407ce82a4703428377d06e4af5c240","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]}
      ]
    },
    "investigation": {
      "startingPoint": {
        "ru": "Прочитайте видение как последовательность: что увидено, что не удалось, что сделано и чего захотел рассказчик. Пусть имя образа появится после этого движения.",
        "en": "Read the vision as a sequence: what is seen, what fails, what is done and what the narrator comes to want. Let the image’s name follow that movement."
      },
      "stakes": {
        "ru": "Смысл преображения входит в желание самого рассказчика. Поэтому вопрос об утверждении должен объяснить не только фигуру пастуха, но и последний вопрос Заратустры о жизни и смерти.",
        "en": "Transformation’s meaning enters the narrator’s own longing. Affirmation’s question must therefore account for both the shepherd’s figure and Zarathustra’s final question about life and death."
      },
      "carryForward": {
        "ru": "Сохраните одну связь между смехом пастуха и припевом о вечности, которую удалось услышать при повторе. Вернитесь к деталям обеих сцен и уточните, на чём она держится.",
        "en": "Keep one connection between the shepherd’s laughter and the eternity refrain that became audible on repetition. Return to both scenes’ details and specify its ground."
      }
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e9175aa652493debe3e712faff10a3df","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      }
    ]
  },
  {
    "id": "memory-others",
    "title": {"ru":"Память и другие","en":"Memory and others"},
    "description": {
      "ru": "Начните с памяти, которая соединяет поступки у Камю, затем проследите детское воспоминание Заратустры: знакомый вой ведёт к новому видению и разным действиям его участников. Через конкретных спутников вернитесь к тому, что означает желать ту же жизнь целиком.",
      "en": "Begin with memory joining actions in Camus, then follow Zarathustra’s childhood recollection: a familiar howl leads to a new vision and its participants’ different acts. Through particular companions, return to what it means to want the same life whole."
    },
    "question": {
      "ru": "Как память открывает возвращение и к чему она приводит за пределами узнавания?",
      "en": "How does memory open return, and where does it lead beyond recognition?"
    },
    "steps": [
      {
        "nodeId": "demo:memory",
        "title": {"ru":"Память соединяет путь","en":"Memory connects a path"},
        "body": {
          "ru": "В «О видении и загадке» память входит иначе: нынешний вой собаки вызывает детскую сцену. Сравните обозрение пути с одним узнаванием, которое ещё поведёт рассказ дальше.",
          "en": "In “The Vision and the Enigma,” memory enters differently: the present howl recalls a childhood scene. Compare surveying a path with one recognition that will carry the narrative onward.",
          "cite": [
            {"w":"le-mythe-de-sisyphe","loc":"subtle"},
            {"w":"le-mythe-de-sisyphe","loc":"joy"},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-7288d1aabe7691c1bc8d3619c3e8ef24"],"s":106}
          ]
        },
        "question": {"ru":"Что соединяется памятью в каждом из двух мест?","en":"What does memory connect in each passage?"},
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"},{"w":"le-mythe-de-sisyphe","loc":"joy"}]
          },
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:remembered-return",
        "title": {"ru":"Узнать знакомый вой","en":"Recognise a familiar howl"},
        "body": {
          "ru": "Настоящая сцена обретает узнаваемую деталь прежде, чем рассказчик понимает её нынешний повод. Возвращение здесь начинается со звука, соединяющего разные положения самого слушающего.",
          "en": "The present scene acquires a recognised detail before the narrator understands its present occasion. Return begins with a sound connecting different positions of the hearer himself.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-36ad02a1d9377c98a649af5994a2c41f"]}
          ]
        },
        "question": {
          "ru": "Почему узнавание звука ещё не даёт понимания нынешней сцены?",
          "en": "Why does recognising the sound not yet explain the present scene?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:attention",
        "title": {"ru":"Следовать за зовом собаки","en":"Follow the dog’s call"},
        "body": {
          "ru": "Знакомое направляет внимание к новому предмету. Проследите этот переход, сохранив различие между воспоминанием рассказчика и ужасом собаки.",
          "en": "The familiar directs attention toward a new object. Follow the transition while distinguishing the narrator’s memory from the dog’s terror.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-e9175aa652493debe3e712faff10a3df"]},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"],"s":0,"e":51}
          ]
        },
        "question": {"ru":"Какое новое внимание рождается из узнавания?","en":"What new attention arises from recognition?"},
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:shared-world",
        "title": {"ru":"Разные участники одной сцены","en":"Different participants in one scene"},
        "body": {
          "ru": "Ни один из этих ходов отдельно не описывает всю сцену. Общий мир видения можно прочитать как связь действий, в которой участники имеют разные места.",
          "en": "No one act describes the whole scene. The vision’s shared world can be read as connected actions in which participants occupy different positions.",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e9175aa652493debe3e712faff10a3df","tos.text-unit.sid-e9175aa652493debe3e712faff10a3df"],"s":104,"e":123},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"],"s":0},
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"],"s":2,"e":47}
          ]
        },
        "question": {
          "ru": "Что произошло бы с описанием сцены, если оставить только действие рассказчика?",
          "en": "What would happen to the scene’s description if only the narrator’s action remained?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e9175aa652493debe3e712faff10a3df","tos.text-unit.sid-e9175aa652493debe3e712faff10a3df"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
              {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:whole-part",
        "title": {"ru":"Целое с этими спутниками","en":"A whole with these companions"},
        "body": {
          "ru": "После видения пастуха вопрос становится конкретнее: какие отношения делают целое этой жизнью?",
          "en": "After the shepherd’s vision, the question becomes more concrete: which relations make the whole this life?",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
          ]
        },
        "question": {
          "ru": "Почему перечисление спутников предшествует словам о той же жизни?",
          "en": "Why are the companions listed before the same-life formula?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]}
            ]
          }
        ]
      },
      {
        "nodeId": "same-life",
        "title": {"ru":"Та же жизнь в большом и малом","en":"The same life in large and small details"},
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"],"s":0,"e":110},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-03419225d5cdf6f8a5e19876524d7a31"]}
          ]
        },
        "question": {
          "ru": "Как повтор задачи входит в тождество жизни?",
          "en": "How does the task’s recurrence enter life’s identity?"
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
        "nodeId": "demo:existential",
        "title": {"ru":"Хотеть всего хода","en":"Wanting the entire course"},
        "body": {
          "ru": "Память привела маршрут к узнаванию, но испытание теперь обращено ко всему составу и порядку жизни.",
          "en": "Memory brought the route to recognition, but the trial now concerns life’s full contents and order.",
          "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
        },
        "question": {
          "ru": "Что требуется от желания всей жизни сверх узнавания её отдельных возвращений?",
          "en": "What does wanting an entire life require beyond recognising particular returns?"
        },
        "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}]
      }
    ],
    "transitions": [
      {
        "from": "demo:memory",
        "to": "demo:remembered-return",
        "edgeId": "atlas:memory:develops:remembered-return",
        "body": {
          "ru": "От памяти, собирающей путь, переходим к одному конкретному узнаванию.",
          "en": "From memory gathering a path, move to a particular recognition."
        }
      },
      {
        "from": "demo:remembered-return",
        "to": "demo:attention",
        "edgeId": "atlas:remembered-return:develops:attention",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]}
          ]
        }
      },
      {
        "from": "demo:attention",
        "to": "demo:shared-world",
        "edgeId": "atlas:attention:develops:shared-world",
        "body": {
          "ru": "Новое внимание открывает разные действия участников видения.",
          "en": "New attention opens the vision’s participants and their different acts."
        }
      },
      {
        "from": "demo:shared-world",
        "to": "demo:whole-part",
        "edgeId": "atlas:shared-world:questions:whole-part",
        "body": {
          "ru": "Связь участников возвращает вопрос к составу целой жизни.",
          "en": "The participants’ connection returns the question to a whole life’s contents."
        }
      },
      {
        "from": "demo:whole-part",
        "to": "same-life",
        "edgeId": "atlas:same-life:develops:whole-part",
        "body": {
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
          ]
        }
      },
      {
        "from": "same-life",
        "to": "demo:existential",
        "edgeId": "atlas:existential:interprets:same-life",
        "body": {
          "ru": "Точность повторяемого становится условием вопроса о желании.",
          "en": "Recurrence’s exactness becomes the condition for desire’s question."
        }
      }
    ],
    "conclusion": {
      "ru": "Память у Камю соединяет поступки в судьбу; в видении Заратустры узнавание открывает новый предмет внимания. Сцена пастуха и спутники воображаемой жизни показывают, как целое образуется через отношения. Вопрос §341 продолжает это чтение: что значит желать весь ход жизни, когда узнавание дало лишь отдельный вход в него?",
      "en": "Memory in Camus joins actions into fate; in Zarathustra’s vision, recognition opens a new object of attention. The shepherd’s scene and the imagined life’s companions show a whole formed through relations. Section 341 continues the reading: what does wanting a life’s entire course mean when recognition supplied only one entrance into it?",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]},
        {"w":"le-mythe-de-sisyphe","loc":"subtle"}
      ]
    },
    "investigation": {
      "startingPoint": {
        "ru": "Начните с различия двух движений памяти: обозреть соединённую жизнь у Камю и услышать знакомый вой у Заратустры. Оба ведут к вопросу о своём пути через разный масштаб.",
        "en": "Begin with two movements of memory: surveying a connected life in Camus and hearing a familiar howl in Zarathustra. Both approach one’s path on different scales."
      },
      "stakes": {
        "ru": "Узнавание способно продолжить внимание к тому, что ещё не было увидено. От этого перехода зависит, останется ли возвращение лишь знакомой деталью или откроет вопрос о связях целого.",
        "en": "Recognition can carry attention toward what has not yet been seen. This transition determines whether return stays a familiar detail or opens the whole’s connections."
      },
      "carryForward": {
        "ru": "Сопоставьте луну из детского воспоминания и лунный свет в речи демона. Объясните, какую роль подробность играет в узнавании и какую — в требовании вернуть всю жизнь.",
        "en": "Compare the moon in childhood memory with the moonlight in the demon’s speech. Explain a detail’s role in recognition and in demanding an entire life’s return."
      }
    },
    "grounds": [
      {
        "ref": "camus-sisyphus",
        "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"},{"w":"le-mythe-de-sisyphe","loc":"joy"}]
      },
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      },
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}
    ]
  },
  {
    "id": "thinkers-necessity",
    "title": {"ru":"Необходимость у разных мыслителей","en":"Necessity across thinkers"},
    "description": {
      "ru": "От различения подвластного у Эпиктета перейдите к участию действующего в судьбе. Затем сопоставьте хотение судьбы у Заратустры, свободу из собственной природы у Спинозы и достаточное основание выбора мира у Лейбница. Каждый вход уточняет свой предмет необходимости.",
      "en": "From Epictetus’s distinction of what is up to us, move to an agent’s participation in fate. Then compare Zarathustra’s wanting fate, Spinoza’s freedom from one’s own nature and Leibniz’s sufficient reason for world selection. Each entry specifies its own object of necessity."
    },
    "question": {
      "ru": "Что объясняет необходимость в действии, судьбе и выборе мира?",
      "en": "What does necessity explain in action, fate and world selection?"
    },
    "steps": [
      {
        "nodeId": "demo:epictetus",
        "title": {"ru":"Начать с двух перечней","en":"Begin with two lists"},
        "body": {
          "ru": "Вопрос свободы получает определённый предмет через то, что человек считает своим.",
          "en": "Freedom gains a definite object through what a person takes as their own.",
          "cite": [
            {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":177},
            {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":347,"e":468}
          ]
        },
        "question": {
          "ru": "Почему ошибочное присвоение внешнего становится внутренней помехой?",
          "en": "Why does wrongly taking the external as one’s own become an inward obstruction?"
        },
        "grounds": [
          {
            "ref": "epictetus1",
            "cite": [
              {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:agency",
        "title": {"ru":"Участие из собственного действия","en":"Participation through one’s own activity"},
        "body": {
          "ru": "Сопоставьте область собственного действия с принадлежностью к причинной связи: это разные способы назвать участие.",
          "en": "Compare one’s own activity with membership in a causal nexus: these name participation differently.",
          "cite": [
            {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":53,"e":128},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"],"s":76,"e":110},
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"]}
          ]
        },
        "question": {
          "ru": "Совпадает ли «своё» с тем, чему человек служит причиной?",
          "en": "Does one’s own coincide with what a person causes?"
        },
        "grounds": [
          {
            "ref": "epictetus1",
            "cite": [
              {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":99,"e":128}
            ]
          },
          {
            "ref": "z-convalescent",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:fate",
        "title": {"ru":"Как судьба становится своей","en":"How fate becomes one’s own"},
        "body": {
          "ru": "Принадлежность пути себе получает различие между сделанным, обозреваемым и желаемым.",
          "en": "One’s path belonging to oneself differs across what is done, surveyed and wanted.",
          "cite": [
            {"w":"le-mythe-de-sisyphe","loc":"subtle"},
            {"w":"le-mythe-de-sisyphe","loc":"joy"},
            {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"],"s":93,"e":135}
          ]
        },
        "question": {
          "ru": "Как желание судьбы соотносится с узнаванием сделанного своим?",
          "en": "How does wanting fate relate to recognising what was done as one’s own?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"},{"w":"le-mythe-de-sisyphe","loc":"joy"}]
          },
          {
            "ref": "z-happy-isles",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:free-necessary",
        "title": {"ru":"Поправка воли","en":"Willing’s correction"},
        "body": {
          "ru": "Перечитайте обе формулировки на «Блаженных островах»: воля названа судьбой, затем воля хочет такой судьбы. Поправка сохраняет необходимость объяснить отношение хотения к уделу. Творчество и становление вокруг неё позволяют спросить, какую свободу ищет это движение воли.",
          "en": "Reread both formulations on “The Happy Isles”: willing is called fate, then the will wants such a fate. The correction retains the need to explain willing’s relation to its lot. The surrounding creation and becoming ask what freedom this movement seeks."
        },
        "question": {
          "ru": "Что изменилось в отношении к судьбе после поправки?",
          "en": "What changes in the relation to fate after the correction?"
        },
        "grounds": [
          {
            "ref": "z-happy-isles",
            "cite": [
              {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-f39f3ace3b6a90408ec2f3f5c98a7a86"]}
            ]
          }
        ]
      },
      {
        "nodeId": "demo:necessity",
        "title": {"ru":"Источник определения","en":"Determination’s source"},
        "body": {
          "ru": "Вопрос теперь касается того, откуда действует вещь, а не одного присутствия необходимости.",
          "en": "The question now concerns where a thing acts from, beyond necessity’s mere presence.",
          "cite": [{"w":"ethica","loc":"I.D7"}]
        },
        "question": {
          "ru": "Что нужно знать о природе вещи, чтобы назвать её действие свободным?",
          "en": "What must be known about a thing’s nature to call its action free?"
        },
        "grounds": [{"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}]
      },
      {
        "nodeId": "demo:spinoza",
        "title": {"ru":"Собственная сила вещи","en":"A thing’s own power"},
        "body": {
          "ru": "Поставьте определение свободы рядом с III.6. Это сравнение уточняет собственное начало действия, оставляя вопрос о том, как вещь соотносится с другими причинами.",
          "en": "Place freedom’s definition beside III.6. The comparison specifies action’s own source while leaving how a thing relates to other causes in question.",
          "cite": [{"w":"ethica","loc":"III.P6"}]
        },
        "question": {
          "ru": "Как «насколько от неё зависит» уточняет собственную силу вещи?",
          "en": "How does “as far as it can by its own power” specify a thing’s power?"
        },
        "grounds": [
          {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]},
          {"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]}
        ]
      },
      {
        "nodeId": "demo:leibniz",
        "title": {"ru":"Основание выбора вселенной","en":"A reason for choosing a universe"},
        "body": {
          "ru": "Рядом со Спинозой сравните работу основания: определять действие из природы или объяснять выбор существующего целого.",
          "en": "Beside Spinoza, compare grounding’s work: determining action from nature or explaining an existing whole’s selection.",
          "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"}]
        },
        "question": {
          "ru": "Почему возможность многих миров заставляет спрашивать об основании этого?",
          "en": "Why do many possible worlds prompt the question of this one’s reason?"
        },
        "grounds": [
          {
            "ref": "leibniz53-55",
            "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
          },
          {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}
        ]
      },
      {
        "nodeId": "demo:possible-worlds",
        "title": {"ru":"Возможное внутри объяснения","en":"Possibility within the explanation"},
        "body": {
          "ru": "Возможность здесь принадлежит объяснению целого. Вернитесь с этим масштабом к Эпиктету: его вопрос о подвластном обращён к иной области действия.",
          "en": "Possibility belongs here to an explanation of the whole. Return with this scale to Epictetus: his question of what is up to us addresses another field of activity.",
          "cite": [{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
        },
        "question": {
          "ru": "Как меняется смысл выбора при переходе от собственного суждения к возможной вселенной?",
          "en": "How does choosing change from one’s own judgement to a possible universe?"
        },
        "grounds": [
          {"ref":"leibniz53-55","cite":[{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]},
          {
            "ref": "epictetus1",
            "cite": [
              {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"],"s":66,"e":98}
            ]
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "demo:epictetus",
        "to": "demo:agency",
        "edgeId": "atlas:epictetus:develops:agency",
        "body": {
          "ru": "Перечни Эпиктета позволяют уточнить, что здесь названо собственным действием.",
          "en": "Epictetus’s lists specify what is called one’s own activity."
        }
      },
      {
        "from": "demo:agency",
        "to": "demo:fate",
        "edgeId": "atlas:agency:develops:fate",
        "body": {
          "ru": "Участие в причинной связи открывает вопрос о личной судьбе.",
          "en": "Participation in a causal nexus opens personal fate’s question."
        }
      },
      {
        "from": "demo:fate",
        "to": "demo:free-necessary",
        "edgeId": "atlas:free-necessary:questions:fate",
        "body": {
          "ru": "От сделанного и обозреваемого пути возвращаемся к хотению судьбы.",
          "en": "From a path made and surveyed, return to wanting fate."
        }
      },
      {
        "from": "demo:free-necessary",
        "to": "demo:necessity",
        "edgeId": "atlas:necessity:develops:free-necessary",
        "body": {
          "ru": "Вопрос свободы уточняем через источник определения к действию.",
          "en": "Specify freedom through determination to act’s source."
        }
      },
      {
        "from": "demo:necessity",
        "to": "demo:spinoza",
        "edgeId": "atlas:spinoza:compares:necessity",
        "body": {
          "ru": "Определение собственной природой сопоставляем с собственной силой вещи.",
          "en": "Compare determination by one’s own nature with a thing’s own power."
        }
      },
      {
        "from": "demo:spinoza",
        "to": "demo:leibniz",
        "edgeId": "atlas:spinoza:compares:leibniz",
        "body": {
          "ru": "От основания действия переходим к основанию выбора мира.",
          "en": "From action’s ground, move to the ground of world selection."
        }
      },
      {
        "from": "demo:leibniz",
        "to": "demo:possible-worlds",
        "edgeId": "atlas:leibniz:develops:possible-worlds",
        "body": {"cite":[{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"}]}
      }
    ],
    "conclusion": {
      "ru": "У Эпиктета различение касается собственных суждений и стремлений; у Спинозы — источника определения к действию; у Лейбница — основания существования одного из возможных миров. Переход через судьбу показал, как смена предмета меняет и смысл необходимости. Продолжить сравнение можно точным вопросом: какое из этих оснований нужно, чтобы объяснить связь хотения и совершённого?",
      "en": "Epictetus distinguishes one’s own judgements and impulses; Spinoza determination to act’s source; Leibniz the reason for one possible world’s existence. Passing through fate showed how a changed object also changes necessity’s meaning. Continue with a precise question: which ground is needed to explain willing’s relation to what is done?"
    },
    "investigation": {
      "startingPoint": {
        "ru": "Прочитайте два перечня Эпиктета прежде общего слова «свобода». Затем сохраняйте вопрос о том, что каждый следующий текст называет своим предметом действия и объяснения.",
        "en": "Read Epictetus’s two lists before the general word freedom. Then retain what each following text takes as its object of action and explanation."
      },
      "stakes": {
        "ru": "Общий словарь скрывает смену масштаба: собственное суждение, природа вещи, личная судьба, возможная вселенная. Содержательное сравнение должно пройти эти переходы, чтобы связать основания.",
        "en": "Shared words can conceal changing scale: one’s judgement, a thing’s nature, personal fate, a possible universe. A substantive comparison traverses these transitions to connect grounds."
      },
      "carryForward": {
        "ru": "Сформулируйте для Эпиктета, Спинозы и Лейбница по одному вопросу, на который отвечает выбранный фрагмент. Затем выберите два ответа и объясните, какое дополнительное рассуждение могло бы связать их.",
        "en": "For Epictetus, Spinoza and Leibniz, formulate one question answered by the selected passage. Then choose two answers and explain what further argument could connect them."
      }
    },
    "grounds": [
      {
        "ref": "epictetus1",
        "cite": [
          {"w":"enchiridion","u":["tos.text-unit.sid-28426296ab784fd6babdd15990300f38","tos.text-unit.sid-28426296ab784fd6babdd15990300f38"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f"]}
        ]
      },
      {
        "ref": "camus-sisyphus",
        "cite": [{"w":"le-mythe-de-sisyphe","loc":"subtle"},{"w":"le-mythe-de-sisyphe","loc":"joy"}]
      },
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      },
      {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]},
      {"ref":"spinoza3p6","cite":[{"w":"ethica","loc":"III.P6"}]},
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
      }
    ]
  }
];
