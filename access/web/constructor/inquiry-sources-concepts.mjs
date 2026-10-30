/** Authored source-grounded inquiry of the sources and concepts; source bytes remain separate.
 * Own words are kept only for what a quotation cannot give (a reading, a
 * reconstructed argument, a distinction, a connection, a question, a method,
 * context). What a work itself says is never retold: a form cites the place
 * instead ({ru, en, cite}), and the stand shows the work's own text there —
 * the original and, through the alignment of the witnesses, the reader's
 * language (bonsai/zarathustra/witnesses.json, fragments.json). */
export const INQUIRY_SOURCES_CONCEPTS = {
  "demo:agency": {
    "argument": {
      "ru": "Способность действовать раскрывается здесь через различие поступков и их носителей: Заратустра пытается извлечь и призывает, пастух кусает и преображается. Рассказчик слышит новый смех и жаждет его; действие другого ещё не делает это преображение его собственным.",
      "en": "Agency appears here through the distinction between acts and their bearers: Zarathustra tries to extract and calls out; the shepherd bites and is transformed. The narrator hears the new laughter and longs for it; another’s act has not yet made that transformation his own.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e"],"e":97},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"],"s":98},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Последовательность позволяет различить действующих и результат каждого действия.",
          "en": "The sequence distinguishes the agents and the result of each act."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      }
    ],
    "question": {
      "ru": "Что в освобождении пастуха принадлежит крику Заратустры, а что становится возможным только в самом укусе?",
      "en": "What in the shepherd’s release belongs to Zarathustra’s cry, and what becomes possible only in the bite itself?"
    },
    "experiment": {
      "setup": {
        "ru": "Перечитайте финал главы и выпишите глаголы, относящиеся к Заратустре, затем к пастуху. Сравните действия до крика, сам крик и то, что происходит после укуса.",
        "en": "Reread the chapter’s ending and list the verbs belonging to Zarathustra, then those belonging to the shepherd. Compare the actions before the cry, the cry itself and what happens after the bite."
      },
      "question": {
        "ru": "Как распределение этих действий меняет чтение сцены, если не сводить её заранее к одному символическому значению?",
        "en": "How does the distribution of these acts change the scene’s reading when no single symbolic meaning is assigned to it in advance?"
      }
    }
  },
  "demo:creation": {
    "argument": {
      "ru": "В конце «О сострадательных» эта линия получает другую форму: великая любовь стремится создать то, что любит, и Заратустра называет созидающих твёрдыми. Созидание здесь связано с преобразующей любовью и собственной волей, а не с получением нового разрешения от дракона.",
      "en": "At the end of The Pitiful, this line takes another form: great love seeks to create what it loves, and Zarathustra calls creators hard. Creation here involves transforming love and a will of one’s own, rather than gaining another permission from the dragon.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1ff6168fe5e0cefdab5c0553114aa8af","tos.text-unit.sid-1ff6168fe5e0cefdab5c0553114aa8af"],"e":125},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-1444d006a038716b06a33646bf0b0439","tos.text-unit.sid-a25836b92a53f9f127019fa8fa725a25"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]},
        {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c","tos.text-unit.sid-2e70c0417788cd9ae6a9d2d112a7670a"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-b8e40296ea25dc2f07dd00a6b704d938","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
        ]
      },
      {
        "ref": "z-pitiful",
        "focus": {
          "ru": "Это основание связи созидания с преобразующей любовью.",
          "en": "This grounds the connection between creation and transforming love."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.03","u":["tos.text-unit.sid-fe959c0f966ea242335efe53f13d134c","tos.text-unit.sid-2e70c0417788cd9ae6a9d2d112a7670a"]}
        ]
      }
    ],
    "question": {
      "ru": "Что ребёнок способен начать благодаря своему «да», чего ещё не совершает львиное «я хочу»?",
      "en": "What can the child begin through its Yes that the lion’s “I will” has not yet accomplished?"
    }
  },
  "demo:amor-fati": {
    "argument": {
      "ru": "Amor fati возникает внутри перехода от учения видеть к становлению утверждающим. Поэтому ход афоризма важен целиком: необходимое становится предметом любви через изменение способа видеть и оценивать, а не через доказательство, что всякое событие уже прекрасно.",
      "en": "Amor fati arises within the movement from learning to see toward becoming an affirmer. The movement of the whole aphorism matters: necessity becomes an object of love through a change in seeing and valuing, rather than through a demonstration that every event is already beautiful.",
      "cite": [{"w":"die-froehliche-wissenschaft","loc":"276"}]
    },
    "grounds": [
      {
        "ref": "gs276",
        "focus": {
          "ru": "Последовательность §276 связывает amor fati с работой над видением и оценкой.",
          "en": "The sequence of §276 connects amor fati with work on seeing and valuing."
        },
        "cite": [{"w":"die-froehliche-wissenschaft","loc":"276"}]
      }
    ],
    "question": {
      "ru": "Как соотносятся в этом пожелании видеть прекрасное, делать прекрасное и оставлять отведение взгляда единственным отрицанием?",
      "en": "How do seeing beauty, making things beautiful and keeping looking away as the sole negation belong together in this wish?"
    }
  },
  "work": {
    "argument": {
      "ru": "В «Выздоравливающем» движение к речи проходит через болезнь, молчание и песни; звери связывают его судьбу с возвещением возвращения. Эти места образуют путь от потребности отдавать к созреванию для своего дела; вопрос о слушателях меняется вместе с самим говорящим.",
      "en": "In The Convalescent, the movement toward speech passes through illness, silence and songs; the animals connect his fate with proclaiming recurrence. These passages trace a movement from the need to give toward ripeness for his work; the question of listeners changes with the speaker himself.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46"],"s":116},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb","tos.text-unit.sid-775c1304b982084134991ec1bc6c84bb"],"s":7},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-778b42e35fce3f7979a96c19045ceb9c","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]},
        {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626","tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626"]},
        {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff","tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff"]},
        {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-9b75d486b97d98d1d08ae0e45159fd04","tos.text-unit.sid-9b75d486b97d98d1d08ae0e45159fd04"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-prologue",
        "focus": {
          "ru": "Отсюда начинается связь избытка с нисхождением к людям.",
          "en": "Here begins the connection between abundance and descent to people."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Говорение связывается с выздоровлением и ещё предстоящей задачей.",
          "en": "Speech is connected with recovery and a task still ahead."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c"]}
        ]
      },
      {
        "ref": "z-sign",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626","tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626"]},
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-d6a1000390b206c389275e22a2d379ae","tos.text-unit.sid-3aed787db3bc681b14b7a606fac6fe70"]},
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff","tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff"]},
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-9b75d486b97d98d1d08ae0e45159fd04","tos.text-unit.sid-9b75d486b97d98d1d08ae0e45159fd04"]}
        ]
      }
    ],
    "question": {
      "ru": "Что изменилось в обращении к солнцу между первым нисхождением и последним выходом из пещеры?",
      "en": "What has changed in the address to the sun between the first descent and the final departure from the cave?"
    }
  },
  "chapter-p3.r2": {
    "argument": {
      "ru": "Глава меняет способ предъявления мысли — от вопроса к видению и действию — и завершается жаждой смеха, которого рассказчик ещё не обрёл.",
      "en": "The chapter changes how it presents the thought—from question to vision and action—and ends with longing for laughter the narrator has not yet attained.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86","tos.text-unit.sid-27d8a9bb60151a6a7946b0e09a848f75"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-27decc16f1823c3d4dd1cde18d43a8c2","tos.text-unit.sid-996c15507635248b4af03be863bead0f"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Именно порядок сцен задаёт ход прочтения.",
          "en": "The sequence of scenes supplies the reading’s movement."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему после вопросов о возвращении рассказу требуется действие пастуха?",
      "en": "Why does the tale require the shepherd’s act after its questions about recurrence?"
    }
  },
  "moment": {
    "argument": {
      "ru": "Мгновение здесь становится местом, где мысль о бесконечности испытывается на способности включить именно это событие, а не только безымянный круг.",
      "en": "The moment becomes the place where the thought of infinity is tested on whether it includes this very event, rather than only an unnamed circle.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d","tos.text-unit.sid-3254efddd6ccc24efe6bbe5313e7f98d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Фокус — последовательное усиление вопроса.",
          "en": "The focus is the question’s successive strengthening."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      }
    ],
    "question": {
      "ru": "Какую работу в рассуждении делает переход от двух вечных дорог к этому пауку и этому разговору?",
      "en": "What work is done by the move from two eternal paths to this spider and this conversation?"
    }
  },
  "chapter-p3.r13": {
    "argument": {
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4cf0a6689b0b424eaee11980c7fd6789","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-33bda0fcdf4170fb11f4dee205767dcd","tos.text-unit.sid-33bda0fcdf4170fb11f4dee205767dcd"],"e":67},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4","tos.text-unit.sid-711dc06c5cfe15d63a5af8a9195a5638"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4cf0a6689b0b424eaee11980c7fd6789","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему выздоровлению недостаточно повторить уже произнесённую формулу возвращения?",
      "en": "Why is repeating the already spoken formula of recurrence insufficient for recovery?"
    }
  },
  "all-things": {
    "argument": {
      "ru": "Всеобщность возвращения поэтому не остаётся отвлечённым объёмом понятия: именно то, что нельзя из неё исключить, делает мысль тяжёлой.",
      "en": "Recurrence’s universality is therefore more than an abstract scope: precisely what cannot be excluded makes the thought heavy.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-352c1b76dce5bcda6be0af9c5a63dbc7","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется, когда слово «всё» включает именно то, от чего Заратустра устал?",
      "en": "What changes when “all” includes precisely what has wearied Zarathustra?"
    }
  },
  "same-life": {
    "argument": {
      "ru": "Рамка условной речи остаётся существенной: такую формулу произносят звери за Заратустру, а повествование заканчивается его беседой с душой.",
      "en": "The conditional frame remains essential: the animals speak this formula on Zarathustra’s behalf, while the narrative ends with his conversation with his soul.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "counterReading": {
      "text": {
        "ru": "В §341 «Весёлой науки» та же строгость повтора предъявляется иначе: демон обращается прямо к «тебе», обещая прежние боли, радости, мысли и вздохи в том же порядке. Эта форма позволяет читать точное повторение как обращённое к человеку испытание, тогда как речь зверей включена в драму выздоровления.",
        "en": "In The Gay Science §341, the same strictness of repetition is presented differently: a demon addresses “you” directly and announces the same pains, joys, thoughts and sighs in the same order. This form allows exact repetition to be read as a trial addressed to a person, whereas the animals’ speech belongs within the drama of recovery.",
        "cite": [
          {"w":"die-froehliche-wissenschaft","loc":"341"},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      },
      "grounds": [{"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]}]
    },
    "question": {
      "ru": "Как изменяется смысл той же жизни, когда формула звучит как обращение к читателю или как воображаемая речь героя?",
      "en": "How does the meaning of the same life change when its formula addresses a reader or appears as a character’s imagined speech?"
    }
  },
  "dossier": {
    "argument": {
      "ru": "Несколько мест о возвращении образуют исследование благодаря различию своих задач. Сопоставление позволяет увидеть, какие слова переходят между этими формами и какие действия речи меняются. Досье удерживает рядом весть, вопрос и приписанную речь, чтобы их связь стала предметом чтения. Оно не требует заранее распределить места на доказательства и опровержения.",
      "en": "Several passages on recurrence form an inquiry through the difference between their tasks. Comparison reveals which words pass between these forms and how the acts of speech change. The dossier holds message, question and attributed speech together so their connection can be read. It need not assign the passages in advance to proof and refutation.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-9ac5e60b3a9f3e21946551e7043e6f31","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]}
        ]
      }
    ],
    "question": {
      "ru": "Какую связь между этими тремя способами речи обнаруживает совместное чтение?",
      "en": "What connection between these three ways of speaking emerges when they are read together?"
    }
  },
  "demo:narrated-voices": {
    "argument": {
      "ru": "Поэтому местоимение «я» в формуле той же жизни ещё не означает, что повествование передало слово самому герою. Различение голосов сохраняет устройство сцены: учение названо извне, а внутренняя беседа остаётся нераскрытой. Из этого расположения нельзя просто сделать ни согласие героя, ни отрицание им учения.",
      "en": "The pronoun “I” in the same-life formula therefore does not yet mean that the narration has given the protagonist the floor. Distinguishing the voices preserves the scene’s structure: the teaching is named from outside while the inward conversation remains undisclosed. This arrangement does not simply establish either his assent or his rejection.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Грамматические переходы показывают вложенность голосов.",
          "en": "The grammatical transitions show the nesting of voices."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Как изменилось бы чтение последней формулы, если бы убрать из неё «ты сказал бы»?",
      "en": "How would the closing formula read differently if its “thou wouldst say” were removed?"
    },
    "experiment": {
      "setup": {
        "ru": "Прочитайте заключительную речь зверей, каждый раз отмечая, кому принадлежат «мы», «ты» и «я». После неё отдельно прочитайте сообщение рассказчика.",
        "en": "Read the animals’ closing speech, marking who is meant by “we”, “you” and “I” each time. Then read the narrator’s report separately."
      },
      "question": {
        "ru": "Где речь о Заратустре переходит в речь от его первого лица и кто производит этот переход?",
        "en": "Where does speech about Zarathustra become speech in his first person, and who makes that transition?"
      }
    }
  },
  "demo:source-neighbor": {
    "argument": {
      "ru": "Последующие строки не отменяют преображения, а уточняют его место: оно увидено и желанно для рассказчика. Соседство реплик удерживает различие между явившимся образом, высказанной возможностью и совершившейся переменой.",
      "en": "The following lines do not cancel the transformation but locate it: the narrator has seen it and desires it. Adjacent utterances preserve the difference between an image appearing, a possibility voiced and a change accomplished.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-cad70ec5a95e4aa4ebec565f3f3a9d3b","tos.text-unit.sid-339e330ca6a6a19079d4f5a3194cb1f7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Последовательность различает участников сцены.",
          "en": "The sequence distinguishes the scene’s participants."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-ff240a868709abef0da385e34299d2c3","tos.text-unit.sid-b074f36127c044db8c943822e45c643a"]}
        ]
      }
    ],
    "question": {
      "ru": "Что прибавляет продолжение сцены к уже прозвучавшей формуле или увиденному образу?",
      "en": "What does the scene’s continuation add to the formula already spoken or image already seen?"
    }
  },
  "demo:local-return": {
    "argument": {
      "ru": "Образ даёт прочитать возвращение как движение полноты, ищущей, куда излиться.",
      "en": "The image allows return to be read as a movement of fullness seeking where to pour itself out.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46"]},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-ef7b6cee183cdd42c58f082bad5cca6a","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"]},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-87ee03096d4da574543c6c6cdeab9b26","tos.text-unit.sid-497b8b73e66daea4df60db1300f77372"]},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-ef7b6cee183cdd42c58f082bad5cca6a","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]},
        {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-5614742e39f2ce0fb4a3cc9789837f46","tos.text-unit.sid-86b4f442b37cc5ac7c949915868d53dd"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-prologue",
        "focus": {"ru":"Эти образы объясняют мотив ухода из пещеры.","en":"These images explain the departure from the cave."},
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.01","u":["tos.text-unit.sid-87ee03096d4da574543c6c6cdeab9b26","tos.text-unit.sid-43df5a5a008e895ca4063ac931f2eae9"]}
        ]
      }
    ],
    "counterReading": {
      "text": {
        "ru": "Так мотив возвращения в книге получает более определённую направленность, чем просто присутствие среди людей.",
        "en": "Return thus acquires a more determinate direction within the book than simply being among people.",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626","tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626"]},
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff","tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff"]},
          {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-550efec95e443c4bd2b7c2897f5a45c2","tos.text-unit.sid-8d196729b153c49989bdf25466f74ff0"]}
        ]
      },
      "grounds": [
        {
          "ref": "z-sign",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-1d1bd60fc75dd18a304e393dda59d626","tos.text-unit.sid-f6f757db50a8c9d5fdce0e1d7c7f3fff"]},
            {"w":"also-sprach-zarathustra","c":"IV.20","u":["tos.text-unit.sid-550efec95e443c4bd2b7c2897f5a45c2","tos.text-unit.sid-3aed787db3bc681b14b7a606fac6fe70"]}
          ]
        }
      ]
    },
    "question": {
      "ru": "Как желание снова стать человеком связано с избытком, который заставляет покинуть одиночество?",
      "en": "How is the wish to become human again connected with the abundance that drives him from solitude?"
    }
  },
  "demo:gateway": {
    "argument": {
      "ru": "Образ меняет роль — из места встречи дорог становится событием, которое само должно входить в предполагаемое возвращение. Так пространственная картина постепенно требует мыслить возвращение конкретной сцены.",
      "en": "The image changes roles—from the paths’ meeting place to an event that must itself belong to the proposed return. The spatial picture gradually demands a thought of this particular scene recurring.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-317301cd82d35cd0e2d4362b72783488","tos.text-unit.sid-dff926efa83f3da3ee1d67d9a3e0538b"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-e060da93a6971669a066b72b51ff92bf"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-dff926efa83f3da3ee1d67d9a3e0538b","tos.text-unit.sid-e060da93a6971669a066b72b51ff92bf"]}
        ]
      }
    ],
    "question": {
      "ru": "Зачем Заратустра после имени ворот возвращается к вопросу об их прежнем существовании?",
      "en": "Why does Zarathustra return from naming the gateway to asking whether it existed before?"
    }
  },
  "demo:two-paths": {
    "argument": {
      "ru": "В чтении важно удержать эти переходы: бесконечная протяжённость дорог, возможность событий и связь всех вещей выполняют разную работу. Само изображение двух дорог ещё не содержит всех последующих утверждений.",
      "en": "Reading requires keeping these transitions in view: the paths’ infinite extent, possible events and the connection of all things do different work. The picture of two paths does not yet contain every subsequent claim.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-7b662f8453b880f152323256b23c2409"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"],"e":137},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d","tos.text-unit.sid-f7d33d4582dcede3074569cda4da793d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-10678cf0654441cbca37527da1dd9ece","tos.text-unit.sid-aa80ebe6fb9de20b613cb4f53ed7e855"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-aa80ebe6fb9de20b613cb4f53ed7e855"]}
        ]
      }
    ],
    "question": {
      "ru": "Что добавляют вопросы о возможном и связанном к первоначальному образу двух вечностей?",
      "en": "What do the questions about possibility and connectedness add to the initial picture of two eternities?"
    }
  },
  "demo:dwarf": {
    "argument": {
      "ru": "Противостояние касается не только формулы времени, но и способности вынести её значение: карлик уже ответил, тогда как для рассказчика вопрос ещё углубляется.",
      "en": "The opposition concerns both the formula of time and the capacity to bear its meaning: the dwarf has already answered, while the narrator’s question is still deepening.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-27decc16f1823c3d4dd1cde18d43a8c2","tos.text-unit.sid-70e02f3b222af3ebae475f634fd8f539"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-7bb0c3706146f1049d8b36a8192e9967","tos.text-unit.sid-7bb0c3706146f1049d8b36a8192e9967"],"s":90,"e":170},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"],"e":105}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-27decc16f1823c3d4dd1cde18d43a8c2","tos.text-unit.sid-54f0be3cffb65e366d441fbeab603bb5"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"]}
        ]
      }
    ],
    "question": {
      "ru": "На что направлен гнев Заратустры: на содержание ответа, его лёгкость или их соединение?",
      "en": "Is Zarathustra’s anger directed at the answer’s content, its lightness or the two together?"
    }
  },
  "demo:circle": {
    "argument": {
      "ru": "Заратустра не предлагает вместо круга другой геометрической фигуры: дальнейшие вопросы тоже ведут к возвращению. Однако теперь круг должен включить именно это мгновение и именно этот разговор. Повторение образа в разных голосах показывает различие между доступной формулой и тем, что происходит с её носителем, когда возвращение охватывает малейшее в его мире.",
      "en": "Zarathustra does not replace the circle with another geometrical figure: his further questions also lead toward return. Now, however, the circle must include this very moment and conversation. The image’s recurrence in different voices shows a difference between an available formula and what happens to its bearer when recurrence includes the smallest things in his world.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-a22b43ad268d4ed093fabbdcb907f76c"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-40ae6f1839d679c4a2bfd41534583aa6","tos.text-unit.sid-03e021d5617af0fc46e337d494c7436d"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется, когда круг означает возвращение этого разговора, а не только форму времени?",
      "en": "What changes when the circle means the return of this conversation as well as a shape of time?"
    }
  },
  "demo:wheel": {
    "argument": {
      "ru": "Общий образ работает в двух разных сочетаниях. Сопоставление удерживает и движение, и различие его смысла: вечное вращение само по себе не объясняет, что значит первое движение, а первое движение ещё не высказывает учение о возвращении.",
      "en": "The shared image works in two different combinations. Comparison preserves both motion and its different meanings: eternal rotation alone does not explain a first movement, while a first movement does not yet state the teaching of recurrence.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-aa15ff6b6ae52c43bb05fe7316272249"]},
        {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9","tos.text-unit.sid-1823f88f8c05b9d921dda6868c4b3bea"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
        ]
      },
      {
        "ref": "z-metamorphoses",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"I.02","u":["tos.text-unit.sid-78b5795aca827182f53ba266a3271646","tos.text-unit.sid-78b5795aca827182f53ba266a3271646"]}
        ]
      }
    ],
    "question": {
      "ru": "Какое значение приобретает колесо в каждом из двух окружений — возвращения и созидания?",
      "en": "What meaning does the wheel acquire in each setting—recurrence and creation?"
    }
  },
  "demo:animals": {
    "argument": {
      "ru": "Их речь соединяет заботу, знание о его задаче и собственный способ выразить её.",
      "en": "Their speech joins care, knowledge of his task and their own way of expressing it.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca"],"s":249,"e":402},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-524ba7995cb645de150e842f8518f37e","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4","tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-79b07d24f16ccdda8f3382d61c9946da","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-e69966f87353b35045f29ba8af84c770"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняется роль зверей между принесённой пищей, песней и словами, которые они дают самому Заратустре?",
      "en": "How does the animals’ role change between the food they bring, their song and the words they give to Zarathustra himself?"
    }
  },
  "demo:nausea": {
    "argument": {
      "ru": "Выздоровление поэтому нельзя понять как простое признание наличия зла. Оно относится к способности вынести возвращение того человеческого масштаба, от которого Заратустра изнемогает.",
      "en": "Recovery therefore cannot be understood as simply admitting that evil exists. It concerns bearing the return of the human scale that exhausts Zarathustra.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-8a03719e2423c8c51817085f23002bbf"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-e5c2cd07214c95f19a1a4dcf30a074b1","tos.text-unit.sid-37240c18b66cd0884b3095c69df63da3"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-7174d0582726c8e4639d0814b0bb0327","tos.text-unit.sid-9f1471144e729291311298ed75a2a53b"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему вечное возвращение малого человека становится для Заратустры труднее простого знания о человеческом зле?",
      "en": "Why does the small human’s eternal return become harder for Zarathustra than simply knowing human badness?"
    }
  },
  "demo:shepherd": {
    "argument": {
      "ru": "«Выздоравливающий» создаёт сильную внутреннюю перекличку: теперь Заратустра говорит от первого лица о чудовище в своей глотке и откушенной голове. Она позволяет читать пастуха как образ предстоящего преображения Заратустры, сохраняя вопросительную форму самого видения.",
      "en": "The Convalescent creates a strong internal echo: Zarathustra now speaks in the first person of a monster in his throat and its bitten-off head. This supports reading the shepherd as an image of Zarathustra’s coming transformation while preserving the vision’s own interrogative form.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-731a9e34e7c14198ab956f27436becc4"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-47ecea77ae8a30862d06ed7c80ebdf1a","tos.text-unit.sid-86f0bdcb39af8bf302dbd2e4ed48b554"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"],"e":91},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-c1a61ec1014026941c54254a0978115d"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "focus": {"ru":"Это точная внутренняя перекличка действия.","en":"This is the precise internal echo of the act."},
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-c1a61ec1014026941c54254a0978115d"]},
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193"]}
        ]
      }
    ],
    "question": {
      "ru": "Что открывает позднейшее «я откусил», когда возвращаешься к вопросу «кто этот пастух»?",
      "en": "What does the later “I bit” reveal when one returns to the question “who is this shepherd”?"
    }
  },
  "demo:bite": {
    "argument": {
      "ru": "В «Выздоравливающем» Заратустра повторяет историю укуса от первого лица, но добавляет остаток, которого праздничный финал видения не показывает: он всё ещё истощён и болен своим спасением. Два места вместе дают укусу временную глубину. Решительное действие прерывает удушье; рассказ о выздоровлении показывает, что последствия этого действия ещё нужно перенести.",
      "en": "In The Convalescent, Zarathustra repeats the bite in the first person but adds a remainder absent from the vision’s triumphant ending: he is still exhausted and sick with his salvation. Together the passages give the bite temporal depth. The decisive act interrupts choking; the account of recovery shows that its consequences still have to be borne.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-7a283fb2da9d76328d331e581c823e09"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-e66b64066560dd30bcae8af64e9f5193","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-f2eedab19687488999cbf7581dc2b74e","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c1a61ec1014026941c54254a0978115d","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
        ]
      }
    ],
    "question": {
      "ru": "Как слова о болезни собственным спасением меняют чтение торжественного укуса в видении?",
      "en": "How does sickness with one’s own salvation change the reading of the triumphant bite in the vision?"
    }
  },
  "demo:laughter": {
    "argument": {
      "ru": "Вместе эти места позволяют читать смех как образ ещё желаемой лёгкости и преображения. Его значение возникает из укуса, жажды и превращения тяжёлого, а не из одного положительного чувства.",
      "en": "Together these passages support reading laughter as an image of lightness and transformation still desired. Its meaning arises from the bite, longing and the transformation of heaviness, rather than from a single positive feeling.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38407ce82a4703428377d06e4af5c240","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-764c1b2fd96faebc6df31914cb7f0851","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]}
        ]
      }
    ],
    "question": {
      "ru": "Что остаётся ещё совершить тому, кто услышал такой смех, но пока только жаждет его?",
      "en": "What remains to be accomplished by someone who has heard such laughter but still only longs for it?"
    }
  },
  "demo:ring": {
    "argument": {
      "ru": "В «Семи печатях» кольцо получает иной определитель: оно становится брачным кольцом возвращения, обращённым к Вечности. Поэтому переход между двумя кольцами содержателен: формула целого становится песней о желании быть с Вечностью связанным. Читатель может проследить, как названные в условных строфах возможности жизни подводят к припеву.",
      "en": "In The Seven Seals, the ring acquires another qualifier: it becomes the wedding ring of recurrence addressed to Eternity. The transition between the two rings therefore matters: a formula of the whole becomes a song of wanting to be joined with Eternity. Readers can trace how the possibilities of life named in the conditional stanzas lead toward the refrain.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"],"s":80},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-a71211ffc3b2ed2d7ebfc9f5b5285bf2","tos.text-unit.sid-a71211ffc3b2ed2d7ebfc9f5b5285bf2"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-5af9e1110a7579a7427c94403f629e85","tos.text-unit.sid-cc29f340979315938b54f43eb7db749e"]},
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-554c18e1670e833686c158edc8c9a2ee","tos.text-unit.sid-04129b0a35e53ec20ae436790aa0dc1c"]}
        ]
      }
    ],
    "question": {
      "ru": "Как меняет образ кольца переход от речи о существовании к обращению «о Вечность»?",
      "en": "How does the move from speaking about existence to addressing Eternity change the ring’s meaning?"
    }
  },
  "demo:dance": {
    "argument": {
      "ru": "Танец поэтому связывает удовольствие от речи с представлением о движении мира, но переход принадлежит диалогу. Эти сцены позволяют читать танец через то, кто или что танцует, и какую тяжесть движение меняет.",
      "en": "Dance thus connects the pleasure of speech with an account of the world’s movement, but the transition belongs to a dialogue. These scenes invite reading dance through who or what dances and which heaviness the movement changes.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c8e9e850b816f1db51f391e81ec26f51","tos.text-unit.sid-711dc06c5cfe15d63a5af8a9195a5638"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c","tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c"]},
        {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-187d71e13bc9881187035bebd261de0c","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"],"e":137}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3d6666a4b970257643f5fff6371d76c4","tos.text-unit.sid-f4bcee757dd64a17da7a137d3e99cd4c"]}
        ]
      },
      {
        "ref": "z-seven-seals",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.16","u":["tos.text-unit.sid-fa02b841750993c61618111d272a18aa","tos.text-unit.sid-187d71e13bc9881187035bebd261de0c"]}
        ]
      }
    ],
    "question": {
      "ru": "Что меняется между «танцевать над вещами» и «вещи сами танцуют»?",
      "en": "What changes between dancing over things and things themselves dancing?"
    }
  },
  "demo:body-resistance": {
    "argument": {
      "ru": "Телесное состояние задаёт время главы: призыв мысли, её перенесение и возможность говорить о ней не совпадают. Рассказ показывает цену и длительность этой встречи, не делая болезнь отдельным доказательством возвращения.",
      "en": "Bodily condition sets the chapter’s time: summoning the thought, bearing it and becoming able to speak about it do not coincide. The narrative shows the cost and duration of this encounter without making illness a separate proof of recurrence.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4cf0a6689b0b424eaee11980c7fd6789","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4f156d0bad165d016f941204caf55153","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"],"s":52},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-1fdb6b338b3ca379b3fc0d4b9a295d84","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Эта последовательность связывает мысль с длительностью выздоровления.",
          "en": "The sequence connects thought with the duration of recovery."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4cf0a6689b0b424eaee11980c7fd6789","tos.text-unit.sid-f95ccd37604cde247a392bebe2a9ea3e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-62eff042a4f2ea561edfff502e5f37ca","tos.text-unit.sid-524ba7995cb645de150e842f8518f37e"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4f156d0bad165d016f941204caf55153","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-4b6e322412bf32bc28e4e88b067989cd"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему глава отмечает и приятный запах яблока, и ещё необходимую новую лиру?",
      "en": "Why does the chapter mark both the apple’s pleasant scent and the new lyre still needed?"
    }
  },
  "demo:recurrence": {
    "argument": {
      "ru": "Сопоставление сохраняет разные способы предъявления: условное обращение, драматическая загадка и речь об учении не выполняют одну и ту же работу.",
      "en": "Comparison preserves different modes of presentation: a conditional address, a dramatic riddle and speech about a teaching do different work.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-20f9857d42ebc6a252b6b3b83551b13e","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-4f156d0bad165d016f941204caf55153","tos.text-unit.sid-4f156d0bad165d016f941204caf55153"]}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-b8cef20c095784beb2dd363f1a0e98cb","tos.text-unit.sid-f7a488c0d098ab2f7052f4287a60e1d1"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f25baa4e2fb13c32304bd277818b128c","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      }
    ],
    "question": {
      "ru": "Что каждая форма — гипотеза, загадка и речь зверей — даёт понять о возвращении?",
      "en": "What does each form—hypothesis, riddle and the animals’ speech—make intelligible about recurrence?"
    }
  },
  "demo:becoming": {
    "argument": {
      "ru": "Становление в речи зверей сохраняет движение, гибель и новое возникновение внутри возвращающегося целого. Фрагмент Гераклита B30 позволяет сравнить меру изменения с повторением года, но сам по себе не излагает возвращение каждой подробности той же жизни. Близость образов требует такого различения.",
      "en": "Becoming in the animals’ speech preserves motion, perishing and renewed emergence within a returning whole. Heraclitus B30 permits comparison between measured change and the repeated year, but does not itself set out every detail of the same life returning. The images’ proximity requires that distinction.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
        {"w":"heraclitus-book","loc":"B30"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-12097ae74434d5e6f369736855a4ad47","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]}
        ]
      },
      {
        "ref": "heraclitus-cosmos",
        "focus": {
          "ru": "Фрагмент задаёт меру изменения, без перечня возвращающихся событий.",
          "en": "The fragment gives measured change without enumerating returning events."
        },
        "cite": [{"w":"heraclitus-book","loc":"B30"}]
      }
    ],
    "question": {
      "ru": "Что сохраняется в великом годе зверей и что — в мерном огне Гераклита?",
      "en": "What is preserved in the animals’ great year, and what in Heraclitus’s measured fire?"
    }
  },
  "demo:present": {
    "argument": {
      "ru": "Настоящее оказывается событием, из которого можно задать вопрос о всей жизни и в которое предполагаемое возвращение должно снова привести. Здесь внимание к малому имеет определённую работу: оно делает содержание возвращения точным и вовлекает положение самого спрашивающего. Изображение не предлагает покинуть прошлое и будущее, а связывает их в этом месте.",
      "en": "The present becomes an event from which the whole life can be questioned and to which the proposed return must lead again. Attention to small things has a precise task here: it specifies recurrence’s content and involves the questioner’s own position. The image does not propose abandoning past and future but connects them in this place.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-36a3c9915901a496c64276fe2da78079"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-38b6718fc95c36a467127712c215fff0","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"]},
        {"w":"die-froehliche-wissenschaft","loc":"341"}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-36a3c9915901a496c64276fe2da78079","tos.text-unit.sid-55f238a2f371dd7f806cdad1f08bc576"]}
        ]
      },
      {
        "ref": "gs341",
        "focus": {"ru":"Гипотеза охватывает собственное предъявление.","en":"The hypothesis includes its own presentation."},
        "cite": [{"w":"die-froehliche-wissenschaft","loc":"341"}]
      }
    ],
    "question": {
      "ru": "Почему мысль о возвращении дважды включает сам момент, когда её сообщают?",
      "en": "Why does the thought of recurrence twice include the moment when it is communicated?"
    }
  },
  "demo:affirmation": {
    "argument": {
      "ru": "Эта речь даёт утверждению внутреннее основание в связанности радости и страдания; «Выздоравливающий» сохраняет драму того, как трудно Заратустре вместить возвращение малого человека.",
      "en": "The speech gives affirmation an internal ground in the connection of joy and suffering; The Convalescent preserves the drama of Zarathustra’s difficulty in bearing the small human’s return.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"276"},
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565"],"s":92},
        {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-9a3da971437436d2ef97a5e5de89a4b2","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
      ]
    },
    "grounds": [
      {"ref":"gs276","cite":[{"w":"die-froehliche-wissenschaft","loc":"276"}]},
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "z-drunken-song",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-6f8647f0f000b169fcbceb7899b30c59"]}
        ]
      }
    ],
    "question": {
      "ru": "Как связанность всего превращает желание одного мгновения в желание всего вновь?",
      "en": "How does the connection of all things turn wanting one moment into wanting everything anew?"
    },
    "counterReading": {
      "text": {
        "ru": "В «Выздоравливающем» сам Заратустра описывает трудность этого расширения: возвращается также малый человек, которого он не может вынести. Это внутреннее сопротивление мысли остаётся частью пути к утверждению; из него нельзя сразу вывести ни окончательный отказ, ни уже достигнутое «да».",
        "en": "In The Convalescent, Zarathustra himself describes the difficulty of this expansion: the small human whom he cannot bear also returns. This internal resistance remains part of the path toward affirmation; it does not immediately establish either a final refusal or an already accomplished Yes.",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c43345c83250f9dce7b12398075b6e70","tos.text-unit.sid-876a89578dd162d1c3de206ed35206a7"]},
          {"w":"also-sprach-zarathustra","c":"IV.19","u":["tos.text-unit.sid-ea9d997ac39761114127a6c0b1800565","tos.text-unit.sid-ad061883f3b347a6a9f5b29dba107495"]}
        ]
      },
      "grounds": [
        {
          "ref": "z-convalescent",
          "cite": [
            {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f44f3910a461757623937a1e557a79e8","tos.text-unit.sid-1d18f623343990179ed1f4ab76c81bfd"]}
          ]
        }
      ]
    }
  },
  "demo:identity": {
    "argument": {
      "ru": "Сопоставление обнаруживает разные основания тождества — повторение всей связи подробностей и сохранение реки при смене вод.",
      "en": "Comparison reveals different grounds of identity—the repetition of an entire connection of details and the river’s persistence through changing waters.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-9e959c02dfdc678408dd5d20287553c2"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-960b157d45a55690ed2c64a9b9259b1f","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]},
        {"w":"heraclitus-book","loc":"B12"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f4d37d7f5099fd59dc5807052b311239","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]}
        ]
      },
      {
        "ref": "heraclitus-river",
        "focus": {
          "ru": "Основание сравнения — совместное присутствие «тех же» и «иных».",
          "en": "The comparison rests on the simultaneous presence of “same” and “different”."
        },
        "cite": [{"w":"heraclitus-book","loc":"B12"}]
      }
    ],
    "question": {
      "ru": "Как различаются требования к «тому же» в возвращении жизни и в речной формуле?",
      "en": "How do the requirements for “the same” differ between life’s recurrence and the river formula?"
    }
  },
  "demo:fate": {
    "argument": {
      "ru": "В «Выздоравливающем» судьба получает другой, ролевой смысл: звери называют судьбой Заратустры быть первым учителем возвращения, и эта задача становится его опасностью и болезнью. В двух главах судьба связана с тем, чего требует созидание или возвещение; её значение нужно читать вместе с конкретной задачей говорящего.",
      "en": "In The Convalescent, fate acquires another, vocational meaning: the animals call being the first teacher of recurrence Zarathustra’s fate, and this task becomes his danger and illness. In both chapters fate connects with what creation or proclamation demands; its meaning must be read with the speaker’s particular task.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-8633a4e29cd08c654ad688a5aef20edc"]},
        {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-8633a4e29cd08c654ad688a5aef20edc","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c673610b053fce15cebb48c6d3dce05e","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
      ]
    },
    "grounds": [
      {
        "ref": "z-happy-isles",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.02","u":["tos.text-unit.sid-dd4549d204c95af0458c76a477cfa443","tos.text-unit.sid-689b101d0eb2f848e7aaeddf6afe12fc"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-3cf5bcb02f23ef9ddd3bb07697eadc6d","tos.text-unit.sid-58fe19ce9170db311f80286870f4b7ee"]}
        ]
      }
    ],
    "question": {
      "ru": "Как задачу судьбы меняет тот, кто её называет: сам созидающий или звери будущего учителя?",
      "en": "How is the task of fate changed by who names it—the creator himself or the future teacher’s animals?"
    },
    "counterReading": {
      "text": {
        "ru": "Этот текст даёт самостоятельное основание отношения к судьбе; его нельзя молча подставить вместо созидающей воли Заратустры.",
        "en": "The text supplies an independent ground for relating to fate; it cannot silently stand in for Zarathustra’s creating will.",
        "cite": [
          {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
        ]
      },
      "grounds": [
        {
          "ref": "marcus6-44",
          "cite": [
            {"w":"meditations","u":["tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da","tos.text-unit.sid-f4af3567827e484dbe8f6c7ee18fc1da"]}
          ]
        }
      ]
    }
  },
  "demo:necessity": {
    "argument": {
      "ru": "Сопоставление разводит необратимость свершившегося и источник определения действия. Из определения Спинозы можно увидеть, почему необходимость не исключает свободу автоматически, но оно само ещё не разрешает драму воли перед прошлым у Заратустры.",
      "en": "Comparison distinguishes the irreversibility of what has happened from the source determining action. Spinoza’s definition shows why necessity does not automatically exclude freedom, but it does not by itself resolve Zarathustra’s drama of the will before the past.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-04ae7c3916e9390af927106df7b960ca","tos.text-unit.sid-04ae7c3916e9390af927106df7b960ca"],"s":87},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-dc0a4af060b9be065759832a96bf8a51","tos.text-unit.sid-3bc55c352acbd3db6c815b153b801968"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-3d1d58f8435846038b07514d30c5672a","tos.text-unit.sid-a72beb585203bf32c2396d8a5f40c90a"]},
        {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-992df47a90ea6b8344d342589d5f915b","tos.text-unit.sid-ff240a868709abef0da385e34299d2c3"]},
        {"w":"ethica","loc":"I.D7"}
      ]
    },
    "grounds": [
      {
        "ref": "z-redemption",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"II.20","u":["tos.text-unit.sid-5d7585f48e3c3ff7eef88bb204463cc3","tos.text-unit.sid-351ef082d5a4c98dcd346a2cbfc1357f"]}
        ]
      },
      {"ref":"spinoza1d7","cite":[{"w":"ethica","loc":"I.D7"}]}
    ],
    "question": {
      "ru": "Как меняется вопрос о свободе при переходе от неизменимого прошлого к источнику определения действия?",
      "en": "How does the question of freedom change when moving from an unchangeable past to the source determining action?"
    }
  },
  "demo:memory": {
    "argument": {
      "ru": "Память работает как переход между сценами: она связывает нынешний звук с прошлым чувством и меняет направление внимания. Текст не предъявляет воспоминание о прежнем мировом круге; повторение звука и чувства нужно отличать от такого предположения.",
      "en": "Memory works as a passage between scenes: it links the present sound with a past feeling and changes the direction of attention. The text presents no recollection of an earlier cosmic cycle; the repetition of sound and feeling must be distinguished from that supposition.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-2f41253d3959f2b77e74490541d39586"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"],"s":82},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa","tos.text-unit.sid-c9379c7d2c3ff474692d6edefdb7db0c"]},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea","tos.text-unit.sid-3d3e4d77d940f820148e2c057cb3b3ea"],"s":69}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-644ff83c17e4f860c7137f40386012fa"]}
        ]
      }
    ],
    "question": {
      "ru": "Что делает возвращение детского чувства в том месте, где спор о времени превращается в видение пастуха?",
      "en": "What does the return of a childhood feeling do where the dispute about time turns into the shepherd’s vision?"
    }
  },
  "demo:difference": {
    "argument": {
      "ru": "В первой песне зверей все вещи расходятся и снова встречаются, умирают и цветут вновь: возвращение содержит различимые движения. Поэтому изменение внутри жизни и отличие одной возвращённой жизни от другой — разные вопросы. Гераклит B12 помогает спрашивать, какой носитель изменяется и чему приписывается тождество. Такое сравнение не требует добавлять в возвращение нового наблюдателя с накопленной памятью: этого условия прочитанные места не задают.",
      "en": "In the animals’ first song, all things separate and meet again, die and blossom anew: recurrence contains distinguishable movements. Change within a life and a difference between one returned life and another are therefore separate questions. Heraclitus B12 helps ask which bearer changes and to what identity is attributed. This comparison does not require adding a new observer with accumulated memories to recurrence: the passages read here specify no such condition.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4","tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4"]},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7","tos.text-unit.sid-53a3c22e0089643fdc72c205f18814c7"]},
        {"w":"heraclitus-book","loc":"B12"}
      ]
    },
    "grounds": [
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39","tos.text-unit.sid-f6f69805abb8029bbedef25eeb121721"]},
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-f65d9dd3e43902a424a1cb7fbe515ed4","tos.text-unit.sid-ce045055585467cfc09217fef32bfa81"]}
        ]
      },
      {
        "ref": "heraclitus-river",
        "focus": {
          "ru": "Фокус — разные носители различия и тождества в одной формуле.",
          "en": "The focus is the different bearers of difference and identity within one formula."
        },
        "cite": [{"w":"heraclitus-book","loc":"B12"}]
      }
    ],
    "question": {
      "ru": "Чем отличается изменение внутри возвращающегося хода от изменения самого хода при возвращении?",
      "en": "How does change within a returning course differ from the course itself changing when it returns?"
    }
  },
  "demo:counterfactual": {
    "argument": {
      "ru": "В §341 воображение меняет положение слушателя перед собственной жизнью, сохраняя внутри гипотезы все её события и их порядок. В одном тексте возможность открывает вопрос о принятии этой жизни, в другом — об основании осуществления одного мира среди многих. Сам способ предположения уже определяет направление мысли.",
      "en": "In §341, imagination changes the hearer’s position toward life while preserving every event and its order within the hypothesis. In one text possibility opens a question about receiving this life; in the other, about the ground for actualizing one world among many. The form of supposition already directs the thought.",
      "cite": [
        {"w":"die-froehliche-wissenschaft","loc":"341"},
        {"w":"monadologie","loc":"53"},
        {"w":"monadologie","loc":"54"},
        {"w":"monadologie","loc":"55"}
      ]
    },
    "grounds": [
      {"ref":"gs341","cite":[{"w":"die-froehliche-wissenschaft","loc":"341"}]},
      {
        "ref": "leibniz53-55",
        "cite": [{"w":"monadologie","loc":"53"},{"w":"monadologie","loc":"54"},{"w":"monadologie","loc":"55"}]
      }
    ],
    "question": {
      "ru": "Что каждый текст оставляет неизменным и что делает предметом предположения?",
      "en": "What does each text leave unchanged, and what does it make the object of supposition?"
    },
    "experiment": {
      "setup": {
        "ru": "Возьмите §341 и §§53–55. В первом отметьте условие появления демона, содержание его сообщения и реакции; во втором — возможные миры, требование основания и ответ на него.",
        "en": "Take §341 and §§53–55. In the first, mark the demon’s hypothetical appearance, its message and the responses; in the second, mark possible worlds, the demand for a reason and the answer."
      },
      "question": {
        "ru": "Как меняется работа возможности между этими двумя последовательностями?",
        "en": "How does the work of possibility change between these two sequences?"
      }
    }
  },
  "demo:remembered-return": {
    "argument": {
      "ru": "Подготовленный отрывок заканчивается постановкой задачи и условной связью повторения со счастьем; исход берлинского опыта здесь ещё не рассказан. Сопоставление с детским воспоминанием Заратустры различает узнавание уже пережитого и попытку осуществить повторение. Это два направления работы с прошлым, а не готовые синонимы.",
      "en": "The prepared excerpt ends with the task’s formulation and a conditional connection between repetition and happiness; the Berlin trial’s outcome is not yet narrated here. Comparison with Zarathustra’s childhood memory distinguishes recognizing what was experienced from trying to bring repetition about. These are two directions of working with the past, not ready-made synonyms.",
      "cite": [{"w":"gjentagelsen","loc":"opening"}]
    },
    "grounds": [
      {
        "ref": "kierkegaard-repetition",
        "focus": {"ru":"Результаты поездки не входят в этот отрывок.","en":"The journey’s results lie outside this excerpt."},
        "cite": [{"w":"gjentagelsen","loc":"opening"}]
      },
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Это конкретный пример направленного к бывшему воспоминания.",
          "en": "This is a specific instance of recollection directed toward what has been."
        },
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-2f41253d3959f2b77e74490541d39586","tos.text-unit.sid-5e6b25576edbed8b36254d7b586ea0cb"]}
        ]
      }
    ],
    "question": {
      "ru": "Что должно произойти сверх узнавания прошлого, чтобы попытка стала повторением в смысле этого вступления?",
      "en": "What must happen beyond recognizing the past for an attempt to become repetition in this opening’s sense?"
    }
  },
  "demo:attention": {
    "argument": {
      "ru": "В конце «Выздоравливающего» движение иное: звери замолкают и ждут ответа, но Заратустра не замечает их молчания, потому что беседует со своей душой. Два места показывают разные направления внимания, из которых рождается или откладывается речь: вовне к собеседникам и внутрь к ещё происходящей работе души.",
      "en": "At the end of The Convalescent, the movement is different: the animals fall silent and await an answer, but Zarathustra does not notice their silence because he is conversing with his soul. The passages show different directions of attention through which speech emerges or is deferred: outwards toward interlocutors and inwards toward the soul’s continuing work.",
      "cite": [
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86","tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86"],"s":229,"e":576},
        {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86","tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86"],"s":681},
        {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"],"s":322}
      ]
    },
    "grounds": [
      {
        "ref": "z-vision",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.02","u":["tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86","tos.text-unit.sid-4979fb4ab5e7de21fc9d81049b413a86"]}
        ]
      },
      {
        "ref": "z-convalescent",
        "cite": [
          {"w":"also-sprach-zarathustra","c":"III.13","u":["tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881","tos.text-unit.sid-2f2fad8321d98a2755d3ef424bbd1881"]}
        ]
      }
    ],
    "question": {
      "ru": "Почему слушание в одной сцене приводит к рассказу, а внутренняя беседа в другой оставляет ответ несказанным?",
      "en": "Why does listening lead to a story in one scene, while inward conversation leaves the reply unspoken in the other?"
    }
  }
};
