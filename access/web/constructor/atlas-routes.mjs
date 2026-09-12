/** Authored source-grounded reading routes; transitions retain atlas edge identity. */
export const RESEARCH_ROUTES = [
  {
    "id": "fate-given-created",
    "title": {
      "ru": "Судьба: данная и создаваемая",
      "en": "Fate: given and made"
    },
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
        "title": {
          "ru": "Что значит «моя судьба»?",
          "en": "What makes fate mine?"
        },
        "body": {
          "ru": "На «Блаженных островах» Заратустра поправляет себя: сначала воля названа его судьбой, затем воля хочет такой судьбы. У Камю личная судьба появляется через поступки и взгляд памяти. Прочитайте рядом эти два притяжательных жеста: хотеть удела и узнавать связность собственного пути.",
          "en": "On “The Happy Isles,” Zarathustra corrects himself: willing is first called his fate, then his will wants such a fate. In Camus, personal fate appears through actions and memory’s gaze. Read these two gestures of appropriation together: wanting a lot and recognising one’s path as connected."
        },
        "question": {
          "ru": "Как изменяется «моё» между волей и памятью?",
          "en": "How does “mine” change between willing and memory?"
        },
        "grounds": [
          {
            "ref": "z-happy-isles",
            "focus": {
              "ru": "Поправка о воле и судьбе после размышления о творчестве.",
              "en": "The correction concerning will and fate after the reflection on creation."
            }
          },
          {
            "ref": "camus-sisyphus",
            "focus": {
              "ru": "Финальные абзацы: личная судьба, поступки и память.",
              "en": "Final paragraphs: personal fate, actions and memory."
            }
          }
        ]
      },
      {
        "nodeId": "demo:camus",
        "title": {
          "ru": "Поступки, собранные взглядом",
          "en": "Actions gathered by a gaze"
        },
        "body": {
          "ru": "В финале эссе Камю поступки оказываются соединены взглядом памяти и ограничены смертью. Эта формулировка стоит внутри возвращения Сизифа к камню, поэтому личная связность возникает вместе с продолжающимся трудом. Уточните, что память здесь делает с совершённым.",
          "en": "At the essay’s close, Camus joins actions through memory’s gaze and bounds them by death. The formulation stands within Sisyphus’s return to the stone, so personal connectedness arises alongside continuing labour. Specify what memory does here with what has been done."
        },
        "question": {
          "ru": "Как взгляд на последовательность поступков позволяет узнать судьбу своей?",
          "en": "How does surveying a sequence of actions allow fate to be recognised as one’s own?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "focus": {
              "ru": "Финальное обозрение поступков, память и предел смерти.",
              "en": "The final survey of actions, memory and death’s limit."
            }
          }
        ]
      },
      {
        "nodeId": "demo:sisyphus",
        "title": {
          "ru": "Спуск как час сознания",
          "en": "The descent as consciousness’s hour"
        },
        "body": {
          "ru": "Вернитесь к тому месту, где Сизиф спускается с вершины. Камю задерживает взгляд на этой паузе: сознание делает удел трагическим и одновременно открывает превосходство над ним. Камень вновь ждёт внизу; изменение нужно искать в отношении героя к следующему усилию.",
          "en": "Return to Sisyphus descending from the summit. Camus dwells on this pause: consciousness makes the lot tragic while also opening a superiority over it. The stone awaits below again; the change lies in the hero’s relation to the next effort."
        },
        "question": {
          "ru": "Почему ясность возникает именно между двумя подъёмами?",
          "en": "Why does lucidity arise precisely between two ascents?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "focus": {
              "ru": "Спуск, пауза, сознание и возвращение к камню.",
              "en": "Descent, pause, consciousness and the return to the stone."
            }
          }
        ]
      },
      {
        "nodeId": "demo:agency",
        "title": {
          "ru": "Участие в повторяемом",
          "en": "Participating in what repeats"
        },
        "body": {
          "ru": "Сизиф продолжает труд и знает его ход; эти два действия сцена удерживает вместе. В §341 демон обращает повторение к слушающему, а заключительный вопрос должен лечь на каждое действие. Переход меняет форму участия: от обозрения труда к мысли, которая способна преобразить действующего.",
          "en": "Sisyphus continues his labour and knows its course; the scene keeps these acts together. In §341, the demon addresses recurrence to the hearer, and the closing question is to weigh on every action. The transition changes participation’s form: from surveying labour to a thought capable of transforming the agent."
        },
        "question": {
          "ru": "Что меняется, когда знание повтора становится вопросом при действии?",
          "en": "What changes when knowledge of repetition becomes a question accompanying action?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "focus": {
              "ru": "Сознательный спуск и не прекращающееся усилие.",
              "en": "The conscious descent and unceasing effort."
            }
          },
          {
            "ref": "gs341",
            "focus": {
              "ru": "Мысль, которая может преобразить или раздавить; вопрос при каждом действии.",
              "en": "The thought that may transform or crush; the question accompanying every action."
            }
          }
        ]
      },
      {
        "nodeId": "demo:decision",
        "title": {
          "ru": "Решение под тяжестью вопроса",
          "en": "Decision under the question’s weight"
        },
        "body": {
          "ru": "Демон не предлагает улучшенную версию жизни: ничто новое не войдёт в повтор. Затем вопрос переносится на каждое действие слушающего. Прочитайте этот переход внимательно: желание повторения должно изменить отношение к совершаемому при сохранении строгого условия того же.",
          "en": "The demon offers no improved life: nothing new enters the recurrence. The question then moves to every action of the hearer. Read this transition closely: wanting recurrence is to change one’s relation to acting while preserving the same-life condition."
        },
        "question": {
          "ru": "Как вопрос о неизменном повторе может преобразить совершающийся поступок?",
          "en": "How can a question about unaltered recurrence transform an action being performed?"
        },
        "grounds": [
          {
            "ref": "gs341",
            "focus": {
              "ru": "От точного состава возвращаемой жизни к вопросу при всём и каждом действии.",
              "en": "From the recurring life’s exact contents to the question in every action."
            }
          }
        ]
      },
      {
        "nodeId": "demo:counterfactual",
        "title": {
          "ru": "Перед словом «было»",
          "en": "Before the word “was”"
        },
        "body": {
          "ru": "В «Об избавлении» воля наталкивается на то, что уже совершилось: она не может хотеть назад. Рядом с демоном вопрос «иначе» становится точнее. Воображение другого поступка и отношение к уже бывшему имеют разные направления, и дальнейшая речь Заратустры занимается вторым.",
          "en": "In “Redemption,” willing encounters what has already happened: it cannot will backward. Beside the demon, “otherwise” becomes more precise. Imagining another act and relating to what has been point in different directions; Zarathustra’s ensuing speech concerns the latter."
        },
        "question": {
          "ru": "Чего воля хочет от прошлого, когда больше не может его изменить?",
          "en": "What does willing want from the past when it can no longer change it?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "focus": {
              "ru": "Воля перед «это было» и невозможность хотеть назад.",
              "en": "Willing before “it was” and the inability to will backward."
            }
          },
          {
            "ref": "gs341",
            "focus": {
              "ru": "Та же жизнь без нового.",
              "en": "The same life without novelty."
            }
          }
        ]
      },
      {
        "nodeId": "demo:responsibility",
        "title": {
          "ru": "От бессилия к обвинению",
          "en": "From powerlessness to accusation"
        },
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
            "focus": {
              "ru": "Месть воли времени и мысль, называющая существование наказанием.",
              "en": "The will’s revenge against time and the thought naming existence punishment."
            }
          }
        ]
      },
      {
        "nodeId": "demo:creation",
        "title": {
          "ru": "Творящий ответ прошлому",
          "en": "A creative response to the past"
        },
        "body": {
          "ru": "Заратустра противопоставляет мести вопрос о творящем «я так хотел». Речь продолжает спрашивать, научилась ли воля примирению со временем и чему-то большему. Финал маршрута остаётся у этой задачи: понять, как созидание меняет ответ бывшему, не делая бывшее несовершившимся.",
          "en": "Zarathustra brings a creative “I willed it thus” against revenge. The speech keeps asking whether willing has learned reconciliation with time and something beyond it. The route ends at this task: understanding how creation changes the response to what was without undoing it."
        },
        "question": {
          "ru": "Что должно измениться в самой воле, чтобы её ответ прошлому стал творящим?",
          "en": "What must change in willing itself for its response to the past to become creative?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "focus": {
              "ru": "Творящая воля перед прошлым и заключительные вопросы о примирении со временем.",
              "en": "Creative willing before the past and the closing questions about reconciliation with time."
            }
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
        "body": {
          "ru": "У демона знание повтора становится вопросом, сопровождающим каждое действие.",
          "en": "For the demon, knowing recurrence becomes a question accompanying every action."
        }
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
      "en": "Making fate acquires several precise meanings: actions are performed, memory joins them, and willing questions its relation to what has been. Camus and “Redemption” connect these movements with continuing life differently. A further reading can follow the remaining question: how does a creative relation to the past connect with willing the future?"
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
        "focus": {
          "ru": "Начало маршрута: воля хочет такой судьбы.",
          "en": "Route opening: the will wants such a fate."
        }
      },
      {
        "ref": "camus-sisyphus",
        "focus": {
          "ru": "Спуск и финальная связность поступков в памяти.",
          "en": "The descent and actions’ final connectedness in memory."
        }
      },
      {
        "ref": "gs341",
        "focus": {
          "ru": "Повтор как вопрос при действии.",
          "en": "Recurrence as a question accompanying action."
        }
      },
      {
        "ref": "z-redemption",
        "focus": {
          "ru": "От невозможности хотеть назад к творящему ответу прошлому.",
          "en": "From inability to will backward toward a creative response to the past."
        }
      }
    ]
  },
  {
    "id": "moment-time",
    "title": {
      "ru": "Мгновение и время",
      "en": "The moment and time"
    },
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
        "title": {
          "ru": "Прочитать надпись",
          "en": "Read the inscription"
        },
        "body": {
          "ru": "Ворота имеют два лица, а над ними написано «Мгновение». Прежде всякого ответа о времени глава помещает настоящее в пространственную сцену. Прочитайте описание до ответа карлика, сохранив то, о чём Заратустра ещё только спрашивает.",
          "en": "The gateway has two faces and bears “Moment.” Before any answer about time, the chapter places the present in a spatial scene. Read the description before the dwarf’s reply, preserving what Zarathustra has yet only asked."
        },
        "question": {
          "ru": "Почему настоящее получает два лица?",
          "en": "Why does the present receive two faces?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: два лица ворот, вечные дороги и надпись.",
              "en": "§2: the gateway’s two faces, eternal paths and inscription."
            }
          }
        ]
      },
      {
        "nodeId": "demo:two-paths",
        "title": {
          "ru": "Мысленно продолжить дороги",
          "en": "Follow the paths in thought"
        },
        "body": {
          "ru": "Каждая дорога простирается в вечность, и у ворот они противоречат друг другу. Затем Заратустра спрашивает, сохранится ли противоречие, если идти дальше. Следуйте этому вопросу как переходу от увиденного расположения к мысленно продолжаемому пути.",
          "en": "Each path extends into eternity, and they oppose one another at the gateway. Zarathustra then asks whether the opposition remains if one goes further. Follow the question as a move from the arrangement seen to a path continued in thought."
        },
        "question": {
          "ru": "Что изменилось между встречей дорог и предположением об их дальнейшем ходе?",
          "en": "What changes between the paths’ meeting and imagining their further course?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: противоречие дорог и вопрос о дальнейшем движении.",
              "en": "§2: the paths’ opposition and the question of following them further."
            }
          }
        ]
      },
      {
        "nodeId": "demo:temporal-knot",
        "title": {
          "ru": "Уточнить связанность",
          "en": "Specify connectedness"
        },
        "body": {
          "ru": "Карлик отвечает кругом времени, но Заратустра считает ответ слишком лёгким. Теперь мгновение должно влечь за собой все будущие вещи, потому что они крепко связаны. Образ узла обозначает эту новую трудность: выяснить, как из встречи направлений возникло требование связи всего.",
          "en": "The dwarf answers with circular time, but Zarathustra finds the answer too easy. The moment must now draw all future things after it because they are firmly bound. The knot marks this new difficulty: how did meeting directions lead to the demand that everything be connected?"
        },
        "question": {
          "ru": "Что связывает мгновение с тем, что ещё только должно случиться?",
          "en": "What binds the moment to what has yet to happen?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: ответ карлика и вопросы о связанности мгновения со всеми вещами.",
              "en": "§2: the dwarf’s answer and the questions connecting the moment with all things."
            }
          }
        ]
      },
      {
        "nodeId": "demo:necessity",
        "title": {
          "ru": "Два хода необходимости",
          "en": "Two movements of necessity"
        },
        "body": {
          "ru": "У ворот Заратустра спрашивает, не должно ли всё способное случиться уже пройти по вечной дороге и затем вернуться. В «Об избавлении» воля наталкивается на иной предел: невозможность хотеть назад. Сопоставьте необходимость повторения с неподвластностью совершившегося; каждый ход требует собственного объяснения.",
          "en": "At the gateway, Zarathustra asks whether everything capable of happening must already have traversed the eternal path and then return. In “Redemption,” willing encounters another limit: it cannot will backward. Compare recurrence’s necessity with the accomplished past’s resistance; each movement calls for its own account."
        },
        "question": {
          "ru": "Какая связь нужна, чтобы от неподвластного прошлого перейти к необходимому повтору?",
          "en": "What connection is needed to move from an unalterable past to necessary recurrence?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: вопросы о возможном, уже бывшем и возвращении.",
              "en": "§2: questions about the possible, what has been and recurrence."
            }
          },
          {
            "ref": "z-redemption",
            "focus": {
              "ru": "Воля перед «это было».",
              "en": "Willing before “it was”."
            }
          }
        ]
      },
      {
        "nodeId": "demo:free-necessary",
        "title": {
          "ru": "Необходимость собственной природы",
          "en": "One’s own nature’s necessity"
        },
        "body": {
          "ru": "Спиноза в определении 7 первой части называет свободным существующее из необходимости собственной природы и определяющее себя к действию. Принуждение связано с определением другим. Этот вход меняет сам вопрос: необходимо выяснить источник определения, прежде чем называть всякую необходимость несвободой.",
          "en": "In Part I, definition 7, Spinoza calls free what exists from its own nature’s necessity and determines itself to act. Constraint concerns determination by another. This entry changes the question itself: determine the source before calling every necessity unfreedom."
        },
        "question": {
          "ru": "Что означает определять себя к действию при сохранении необходимости?",
          "en": "What does determining oneself to act mean while necessity is retained?"
        },
        "grounds": [
          {
            "ref": "spinoza1d7",
            "focus": {
              "ru": "I, определение 7 целиком: свободное и принуждённое.",
              "en": "I, definition 7 in full: the free and the constrained."
            }
          }
        ]
      },
      {
        "nodeId": "demo:fate",
        "title": {
          "ru": "Хотеть такой судьбы",
          "en": "Wanting such a fate"
        },
        "body": {
          "ru": "На «Блаженных островах» Заратустра сначала называет волю своей судьбой, затем заменяет это утверждение хотением такой судьбы. Поправка стоит после мысли о творчестве, превращении и боли рождения. Вернитесь с ней к свободе и необходимости: отношение к судьбе теперь сформулировано как желание становления.",
          "en": "On “The Happy Isles,” Zarathustra first calls willing his fate, then replaces this with wanting such a fate. The correction follows creation, transformation and birth’s pain. Bring it back to freedom and necessity: the relation to fate is now formulated as desiring becoming."
        },
        "question": {
          "ru": "Почему хотение такой судьбы требует поправить первое определение воли?",
          "en": "Why does wanting such a fate require correcting the first account of willing?"
        },
        "grounds": [
          {
            "ref": "z-happy-isles",
            "focus": {
              "ru": "Творчество, становление и поправка о воле и судьбе.",
              "en": "Creation, becoming and the correction about willing and fate."
            }
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
          "ru": "После ответа карлика вопрос переносится к связанности всех вещей.",
          "en": "After the dwarf’s answer, the question moves to all things’ connectedness."
        }
      },
      {
        "from": "demo:temporal-knot",
        "to": "demo:necessity",
        "edgeId": "atlas:temporal-knot:develops:necessity",
        "body": {
          "ru": "Связанность мгновения вводит вопрос о том, что должно повториться.",
          "en": "The moment’s connectedness introduces what must recur."
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
        "focus": {
          "ru": "От ворот к связанности и вопросам необходимости возвращения.",
          "en": "From the gateway to connection and questions of recurrence’s necessity."
        }
      },
      {
        "ref": "z-redemption",
        "focus": {
          "ru": "Невозможность хотеть назад.",
          "en": "The inability to will backward."
        }
      },
      {
        "ref": "spinoza1d7",
        "focus": {
          "ru": "Необходимость собственной природы в определении свободы.",
          "en": "One’s own nature’s necessity within freedom’s definition."
        }
      },
      {
        "ref": "z-happy-isles",
        "focus": {
          "ru": "Хотение судьбы в контексте творчества.",
          "en": "Wanting fate in creation’s context."
        }
      }
    ]
  },
  {
    "id": "repetition-difference",
    "title": {
      "ru": "Повтор и различие",
      "en": "Repetition and difference"
    },
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
        "title": {
          "ru": "Ни новая, ни лучшая",
          "en": "Neither new nor better"
        },
        "body": {
          "ru": "Звери воображают, что Заратустра сказал бы, если бы пожелал умереть теперь: он вернётся к той же жизни с тем же солнцем, землёй, орлом и змеёй. Перед этим исключены новая, лучшая и лишь подобная жизни. Сохраните всю силу этих исключений как исходную трудность маршрута.",
          "en": "The animals imagine what Zarathustra would say if he wished to die now: he returns to the same life with the same sun, earth, eagle and snake. A new, better or merely similar life is first excluded. Preserve these exclusions’ full force as the route’s initial difficulty."
        },
        "question": {
          "ru": "Что потеряется из мысли о возвращении, если оставить только сходство?",
          "en": "What would recurrence lose if only resemblance were retained?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Конец §2: звери воображают речь о той же жизни и её спутниках.",
              "en": "End of §2: the animals imagine a speech about the same life and its companions."
            }
          }
        ]
      },
      {
        "nodeId": "demo:identity",
        "title": {
          "ru": "Тот же ход подробностей",
          "en": "The same course of particulars"
        },
        "body": {
          "ru": "Демон в §341 перечисляет боли, радости, мысли и вздохи, а затем требует того же порядка. Тождество касается и состава, и последовательности: паук, лунный свет и сам демон тоже входят в него. Теперь вопрос к последующему прочтению точен: какую роль оно даст этому требованию?",
          "en": "In §341, the demon lists pains, joys, thoughts and sighs, then requires the same order. Identity concerns contents and sequence: spider, moonlight and demon belong to it too. The question for the next reading is now precise: what role will it give this demand?"
        },
        "question": {
          "ru": "Почему сохранения главных событий недостаточно для этой формулы?",
          "en": "Why is retaining the main events insufficient for this formula?"
        },
        "grounds": [
          {
            "ref": "gs341",
            "focus": {
              "ru": "Речь демона: состав жизни, мелкие подробности и прежняя последовательность.",
              "en": "The demon’s speech: life’s contents, small particulars and the previous sequence."
            }
          },
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Та же жизнь в большом и малом.",
              "en": "The same life in large and small details."
            }
          }
        ]
      },
      {
        "nodeId": "demo:return-difference",
        "title": {
          "ru": "Что не объясняет цикл?",
          "en": "What does a cycle leave unexplained?"
        },
        "body": {
          "ru": "В «Ницше и философии» Делёз спрашивает, как механический процесс выходит из начального состояния, проходит различия и воспроизводит их. Тождество начала и конца само по себе ещё не выполняет эту объяснительную работу. Вопрос перемещается от описания повтора к основанию его движения.",
          "en": "In “Nietzsche and Philosophy,” Deleuze asks how a mechanical process leaves its initial state, passes through differences and reproduces them. Identity between beginning and ending does not itself do this explanatory work. The question moves from recurrence’s description to the ground of its movement."
        },
        "question": {
          "ru": "Что следует объяснить помимо совпадения начального и конечного состояния?",
          "en": "What needs explaining beyond matching initial and final states?"
        },
        "grounds": [
          {
            "ref": "deleuze-nietzsche-return",
            "focus": {
              "ru": "Печатная с. 49: механический цикл, различия внутри цикла и разнообразие сосуществующих циклов.",
              "en": "Printed p. 49: the mechanical cycle, differences within it and the diversity of coexisting cycles."
            }
          }
        ]
      },
      {
        "nodeId": "demo:difference",
        "title": {
          "ru": "Различающее основание",
          "en": "A differential ground"
        },
        "body": {
          "ru": "Делёз называет волю к власти различающим и генетическим элементом сил. Он связывает её с синтезом, в котором различия воспроизводятся, и прямо оставляет дальнейшую задачу объяснить, как этот синтез образует возвращение. Удержите различие между предложенным основанием и ещё требуемым объяснением.",
          "en": "Deleuze calls will to power forces’ differential and genetic element. He connects it with a synthesis reproducing differences and explicitly leaves the further task of explaining how this synthesis forms recurrence. Retain the distinction between a proposed ground and the explanation still required."
        },
        "question": {
          "ru": "Что именно должно быть воспроизведено, если основание само различающее?",
          "en": "What must be reproduced if the ground is itself differential?"
        },
        "grounds": [
          {
            "ref": "deleuze-nietzsche-return",
            "focus": {
              "ru": "Печатные с. 50–52: элемент сил, принцип синтеза и оставшаяся задача объяснения.",
              "en": "Printed pp. 50–52: forces’ element, synthesis’s principle and the remaining explanatory task."
            }
          }
        ]
      },
      {
        "nodeId": "demo:same-new",
        "title": {
          "ru": "Сделать новым само повторение",
          "en": "Make repetition itself new"
        },
        "body": {
          "ru": "Во введении к «Различию и повторению» Делёз различает извлечение нового из наблюдаемого повтора и превращение самого повторения в предмет воли. Второе он связывает со свободой и её задачей. Прочитайте этот ход рядом с требованием той же жизни: новизна теперь относится и к тому, как повторение хотят.",
          "en": "In the introduction to “Difference and Repetition,” Deleuze distinguishes extracting novelty from observed repetition and making repetition itself willing’s object. He connects the latter with freedom and its task. Read this beside the same-life demand: novelty now concerns how repetition is willed as well."
        },
        "question": {
          "ru": "Как изменяется вопрос о новом, если повторение становится предметом желания?",
          "en": "How does novelty’s question change when repetition becomes desire’s object?"
        },
        "grounds": [
          {
            "ref": "deleuze-repetition",
            "focus": {
              "ru": "Введение, печатная с. 6: сделать новым повторение и сделать его предметом воли.",
              "en": "Introduction, printed p. 6: making repetition new and making it willing’s object."
            }
          },
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Конец §2: та же жизнь без новой или лучшей версии.",
              "en": "End of §2: the same life without a new or better version."
            }
          }
        ]
      },
      {
        "nodeId": "demo:creation",
        "title": {
          "ru": "Ребёнок и самокатящееся колесо",
          "en": "The child and the self-rolling wheel"
        },
        "body": {
          "ru": "Ребёнок в «Трёх превращениях» назван невинностью, забвением, новым началом и самокатящимся колесом. Лев мог добыть свободу для созидания, но только ребёнок даёт нужное ему священное «да». Вернитесь к исходному вопросу через это соседство: новизна, забывание и собственное движение уже соединены в одном образе.",
          "en": "In “The Three Metamorphoses,” the child is innocence, forgetting, a new beginning and a self-rolling wheel. The lion could win freedom for creation, but only the child gives creation its sacred yes. Return to the initial question through this conjunction: novelty, forgetting and self-movement already meet in one image."
        },
        "question": {
          "ru": "Что образ ребёнка позволяет спросить о возвращении, чего не давала одна геометрия круга?",
          "en": "What does the child’s image let us ask about recurrence beyond a circle’s geometry?"
        },
        "grounds": [
          {
            "ref": "z-metamorphoses",
            "focus": {
              "ru": "Лев как добывающий свободу; ребёнок, забывание, начало, колесо и священное «да».",
              "en": "The lion winning freedom; the child, forgetting, beginning, wheel and sacred yes."
            }
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
        "body": {
          "ru": "Критика цикла ведёт к различающему и генетическому элементу сил.",
          "en": "The cycle’s critique leads to forces’ differential and genetic element."
        }
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
      "ru": "Маршрут различил строгость повторяемой жизни, объяснение движения и новизну самого хотения повтора. Делёз переводит вопрос к различающему основанию и задаче воли; ребёнок у Заратустры соединяет новое начало с колесом. Дальнейшее чтение должно показать, как эти ходы объясняют друг друга и где требуют разных значений «того же».",
      "en": "The route distinguishes the recurring life’s strictness, movement’s explanation and novelty in willing recurrence itself. Deleuze turns toward a differential ground and willing’s task; Zarathustra’s child joins a new beginning with a wheel. Further reading must show how these movements illuminate one another and where they require different meanings of the same."
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
        "focus": {
          "ru": "Исходная формула той же жизни.",
          "en": "The initial same-life formula."
        }
      },
      {
        "ref": "gs341",
        "focus": {
          "ru": "Подробности и порядок повторяемого.",
          "en": "Recurrence’s particulars and order."
        }
      },
      {
        "ref": "deleuze-nietzsche-return",
        "focus": {
          "ru": "Печатные с. 49–52: движение, различие и синтез.",
          "en": "Printed pp. 49–52: movement, difference and synthesis."
        }
      },
      {
        "ref": "deleuze-repetition",
        "focus": {
          "ru": "Введение, печатная с. 6: повторение как новизна и предмет воли.",
          "en": "Introduction, printed p. 6: repetition as novelty and willing’s object."
        }
      },
      {
        "ref": "z-metamorphoses",
        "focus": {
          "ru": "Ребёнок как новое начало и самокатящееся колесо.",
          "en": "The child as a new beginning and self-rolling wheel."
        }
      }
    ]
  },
  {
    "id": "affirmation-pain-action",
    "title": {
      "ru": "Утверждение, боль и действие",
      "en": "Affirmation, pain and action"
    },
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
        "title": {
          "ru": "Учиться amor fati",
          "en": "Learning amor fati"
        },
        "body": {
          "ru": "В §276 говорящий хочет научиться видеть необходимое прекрасным, чтобы стать одним из делающих вещи прекрасными. Далее идут желание не обвинять и надежда однажды быть только утверждающим. Прочитайте эту последовательность как движение ещё предстоящего учения.",
          "en": "In §276, the speaker wants to learn to see necessity as beautiful and thereby become one who makes things beautiful. Wishing not to accuse and hoping one day to be only affirmative follow. Read this as the movement of learning still ahead."
        },
        "question": {
          "ru": "Как внутри этого желания соединяются видеть и делать прекрасным?",
          "en": "How are seeing and making beautiful joined within this wish?"
        },
        "grounds": [
          {
            "ref": "gs276",
            "focus": {
              "ru": "§276 целиком: учение взгляда, делание прекрасным, не обвинять и будущее утверждение.",
              "en": "§276 in full: learning to see, making beautiful, not accusing and future affirmation."
            }
          }
        ]
      },
      {
        "nodeId": "demo:affirmation",
        "title": {
          "ru": "От будущего «да» к его объёму",
          "en": "From a future yes to its scope"
        },
        "body": {
          "ru": "Желание однажды стать утверждающим ещё оставляет вопрос, чего хочет это «да». В «Песни опьянения» достаточно обратиться к одному радостному мгновению, чтобы возникло желание возвращения всех вещей. Проследите, как личное желание расширяется в самой речи.",
          "en": "Wanting one day to become affirmative still leaves what this yes wants in question. In “The Drunken Song,” turning to one joyful moment brings forth a desire for all things’ return. Follow the expansion of personal desire within the speech itself."
        },
        "question": {
          "ru": "Почему утверждённое мгновение не остаётся отдельным мгновением?",
          "en": "Why does an affirmed moment not remain an isolated moment?"
        },
        "grounds": [
          {
            "ref": "gs276",
            "focus": {
              "ru": "Заключительное желание однажды стать только утверждающим.",
              "en": "The closing wish one day to be only affirmative."
            }
          },
          {
            "ref": "z-drunken-song",
            "focus": {
              "ru": "§10: одно утверждённое мгновение и связанность всех вещей.",
              "en": "§10: one affirmed moment and all things’ connection."
            }
          }
        ]
      },
      {
        "nodeId": "demo:joy-pain",
        "title": {
          "ru": "Радость хочет себя и всего",
          "en": "Joy wants itself and everything"
        },
        "body": {
          "ru": "В §9 горе хочет наследников и продолжения, тогда как радость хочет себя и вечности. В §§10–11 радость одного мгновения захватывает весь связанный ход вещей, включая горе. Удержите обе ступени: различие хотений и затем расширение одного из них.",
          "en": "In §9, sorrow wants heirs and continuation, whereas joy wants itself and eternity. In §§10–11, one moment’s joy takes in things’ whole connected course, including sorrow. Keep both stages: the differing forms of willing and then one’s expansion."
        },
        "question": {
          "ru": "Как песнь переходит от различия радости и горя к желанию всего?",
          "en": "How does the song move from joy’s difference from sorrow to wanting everything?"
        },
        "grounds": [
          {
            "ref": "z-drunken-song",
            "focus": {
              "ru": "§§9–11: горе, радость, связанность и вечность.",
              "en": "§§9–11: sorrow, joy, connection and eternity."
            }
          }
        ]
      },
      {
        "nodeId": "demo:shared-world",
        "title": {
          "ru": "Встретить страдающего друга",
          "en": "Encounter the suffering friend"
        },
        "body": {
          "ru": "«О сострадательных» меняет масштаб чтения: речь идёт о помощи, стыде страдающего и раненой гордости. Для друга Заратустра предлагает жёсткое ложе, а большая любовь оказывается выше одной жалости. Прочитайте это обращение рядом со всеобщим хотением радости и спросите, как мысль получает определённость в отношении.",
          "en": "“The Pitiful” changes the reading’s scale: help, the sufferer’s shame and wounded pride enter. For a friend, Zarathustra offers a hard bed, while great love goes beyond pity alone. Read this address beside joy’s universal willing and ask how thought becomes definite within a relation."
        },
        "question": {
          "ru": "Что друг должен получить от любви, чего ещё не даёт одна жалость?",
          "en": "What must the friend receive from love beyond what pity alone gives?"
        },
        "grounds": [
          {
            "ref": "z-pitiful",
            "focus": {
              "ru": "Стыд помощи, гордость страдающего, друг и жёсткое ложе.",
              "en": "Help’s shame, the sufferer’s pride, the friend and a hard bed."
            }
          },
          {
            "ref": "z-drunken-song",
            "focus": {
              "ru": "§§10–11: желание всех связанных вещей.",
              "en": "§§10–11: desire for all connected things."
            }
          }
        ]
      },
      {
        "nodeId": "demo:responsibility",
        "title": {
          "ru": "Ответ и действие дара",
          "en": "Response and the gift’s action"
        },
        "body": {
          "ru": "Помощь может сделать принимающего должником; в главе большие обязательства связаны с местью. Это заставляет исследовать сам способ отвечать другому. Затем в «Об избавлении» месть получает другое основание — бессилие воли перед временем; сопоставление уточняет, откуда возникает требование ответа.",
          "en": "Help may make its recipient indebted; the chapter connects great obligations with revenge. This directs inquiry toward the manner of answering another. In “Redemption,” revenge receives another ground—willing’s powerlessness before time; comparison specifies where a demand for response arises."
        },
        "question": {
          "ru": "Как изменяется предмет ответа между тяжестью дара и тяжестью прошлого?",
          "en": "How does response’s object change between a gift’s burden and the past’s burden?"
        },
        "grounds": [
          {
            "ref": "z-pitiful",
            "focus": {
              "ru": "Большие обязательства, ответная месть и разные способы дарить.",
              "en": "Great obligations, revenge in response and different ways of giving."
            }
          },
          {
            "ref": "z-redemption",
            "focus": {
              "ru": "Месть воли времени и объяснение существования наказанием.",
              "en": "The will’s revenge against time and existence explained as punishment."
            }
          }
        ]
      },
      {
        "nodeId": "demo:counterfactual",
        "title": {
          "ru": "Хотеть иначе перед бывшим",
          "en": "Willing otherwise before what was"
        },
        "body": {
          "ru": "В «Об избавлении» воля не может хотеть назад, а её бессилие обращается в обвинение существования. Вопрос о творящем «я так хотел» возникает внутри этой трудности. Прочитайте его рядом с желанием §276 не обвинять даже обвинителей: что должно измениться в отношении воли к прошлому?",
          "en": "In “Redemption,” willing cannot will backward, and its powerlessness turns into accusing existence. The creative “I willed it thus” arises within this difficulty. Read it beside §276’s wish not to accuse even accusers: what must change in willing’s relation to the past?"
        },
        "question": {
          "ru": "Какое хотение остаётся возможным перед тем, что уже совершилось?",
          "en": "What willing remains possible before what has already happened?"
        },
        "grounds": [
          {
            "ref": "z-redemption",
            "focus": {
              "ru": "Невозможность хотеть назад, месть и творящее отношение к бывшему.",
              "en": "Inability to will backward, revenge and a creative relation to what was."
            }
          },
          {
            "ref": "gs276",
            "focus": {
              "ru": "Желание не обвинять даже обвинителей.",
              "en": "The wish not to accuse even accusers."
            }
          }
        ]
      },
      {
        "nodeId": "demo:decision",
        "title": {
          "ru": "Вопрос при каждом действии",
          "en": "A question in every action"
        },
        "body": {
          "ru": "Вернитесь к §341: мысль о повторении должна лечь на каждое действие. После чтения о радости, помощи и мести этот вопрос получает определённый материал: как отношение к происходящему входит в то, что человек совершает? Текст оставляет превращение слушающего условием желания повторения.",
          "en": "Return to §341: recurrence’s thought is to weigh on every action. After joy, help and revenge, the question has definite material: how does a relation to events enter what a person does? The text makes the hearer’s transformation a condition of wanting recurrence."
        },
        "question": {
          "ru": "Что значит хотеть повторения именно этого способа действовать?",
          "en": "What does it mean to want this very way of acting to return?"
        },
        "grounds": [
          {
            "ref": "gs341",
            "focus": {
              "ru": "Мысль, преобразующая слушающего, и вопрос при всём и каждом действии.",
              "en": "The thought transforming the hearer and the question in every action."
            }
          }
        ]
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
      {
        "ref": "gs276",
        "focus": {
          "ru": "Amor fati как учение взгляда и желание не обвинять.",
          "en": "Amor fati as learning to see and wishing not to accuse."
        }
      },
      {
        "ref": "z-drunken-song",
        "focus": {
          "ru": "§§9–11: расширение хотения радости.",
          "en": "§§9–11: joy’s willing expands."
        }
      },
      {
        "ref": "z-pitiful",
        "focus": {
          "ru": "Помощь, стыд, дар, обязательство и любовь к другу.",
          "en": "Help, shame, gift, obligation and love for the friend."
        }
      },
      {
        "ref": "z-redemption",
        "focus": {
          "ru": "Бессилие воли, месть и творящий ответ.",
          "en": "Willing’s powerlessness, revenge and a creative response."
        }
      },
      {
        "ref": "gs341",
        "focus": {
          "ru": "Желание повторения при каждом действии.",
          "en": "Wanting recurrence in every action."
        }
      }
    ]
  },
  {
    "id": "reading-voices",
    "title": {
      "ru": "Чтение и голоса",
      "en": "Reading and voices"
    },
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
        "title": {
          "ru": "Все вещи — для кого?",
          "en": "All things—for whom?"
        },
        "body": {
          "ru": "Перед песней звери уточняют: для думающих, как они, сами вещи танцуют. Затем колесо, год, дом и кольцо связывают уход с возвращением. Начните с этой рамки: всеобщая речь введена через определённый способ думать.",
          "en": "Before the song, the animals specify that for those who think as they do, things themselves dance. Wheel, year, house and ring then join departure with return. Begin with this frame: universal speech is introduced through a particular way of thinking."
        },
        "question": {
          "ru": "Что добавляет к словам о всех вещах оговорка «для тех, кто думает, как мы»?",
          "en": "What does “for those who think as we do” add to the words about all things?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "§2: вступление зверей к песне и последовательность её образов.",
              "en": "§2: the animals’ introduction to the song and its images’ sequence."
            }
          }
        ]
      },
      {
        "nodeId": "demo:animals",
        "title": {
          "ru": "Когда звери начинают говорить",
          "en": "When the animals begin to speak"
        },
        "body": {
          "ru": "После семи дней болезни Заратустра находит приятным запах яблока, и звери решают, что пора обратиться к нему. Они зовут его в мир-сад; он отвечает, что сама их болтовня уже даёт ему такой мир. Забота и речь входят в выздоровление ещё до изложения учения.",
          "en": "After seven days of illness, Zarathustra finds an apple’s scent pleasant, and the animals decide it is time to address him. They invite him into the world as a garden; he replies that their chatter already gives him such a world. Care and speech enter recovery before the teaching is expounded."
        },
        "question": {
          "ru": "Почему приятный запах и приглашение слушать важны для последующей формулы?",
          "en": "Why do the pleasant scent and invitation to listen matter for the formula that follows?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Начало §2: яблоко, приглашение зверей и ответ о мире как саде.",
              "en": "Beginning of §2: the apple, the animals’ invitation and the world-as-garden reply."
            }
          }
        ]
      },
      {
        "nodeId": "demo:voice-reading",
        "title": {
          "ru": "Кто танцует на вещах?",
          "en": "Who dances upon things?"
        },
        "body": {
          "ru": "Заратустра говорит о словах и звуках как призрачных мостах между отдельными мирами душ; благодаря речи человек танцует на вещах. Звери отвечают, что сами вещи танцуют и возвращаются. Различие голосов касается здесь отношения языка к миру, а не только подписи под формулой.",
          "en": "Zarathustra speaks of words and sounds as illusory bridges between souls’ separate worlds; through speech, a person dances upon things. The animals answer that things themselves dance and return. The voices differ over language’s relation to the world, beyond who signs the formula."
        },
        "question": {
          "ru": "Как меняется утверждение при переходе от танцующего говорящего к танцующим вещам?",
          "en": "How does the claim change from a dancing speaker to dancing things?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "§2: речь Заратустры о словах и звуках, затем ответ зверей о танцующих вещах.",
              "en": "§2: Zarathustra on words and sounds, then the animals’ reply about dancing things."
            }
          }
        ]
      },
      {
        "nodeId": "demo:source-neighbor",
        "title": {
          "ru": "Услышать следующий ответ",
          "en": "Hear the following reply"
        },
        "body": {
          "ru": "После песни Заратустра улыбается и называет зверей шарманками. Он рассказывает об откушенной голове чудовища и подчёркивает, что ещё болен собственным избавлением. Соседство реплик ставит вопрос о различии между уже готовой песней и продолжающимся переживанием.",
          "en": "After the song, Zarathustra smiles and calls the animals barrel-organs. He recounts biting off the monster’s head and stresses that he remains ill from his own redemption. The neighbouring speeches distinguish an already completed song from an experience still unfolding."
        },
        "question": {
          "ru": "Что позволяет улыбке и упрёку стоять в одной реплике?",
          "en": "What allows the smile and reproach to inhabit one reply?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "§2: непосредственный ответ на песню, чудовище и продолжающаяся болезнь.",
              "en": "§2: the immediate reply to the song, the monster and continuing illness."
            }
          }
        ]
      },
      {
        "nodeId": "demo:narrated-voices",
        "title": {
          "ru": "Речь прерывается советом петь",
          "en": "Speech interrupted by advice to sing"
        },
        "body": {
          "ru": "Рассказ об отвращении завершается дрожью, и звери не дают Заратустре продолжать. Они дважды советуют перестать говорить и в конце предлагают новую лиру для новых песен. Теперь карта голосов показывает, как разговор сам пытается изменить состояние говорящего.",
          "en": "The disgust account ends in trembling, and the animals stop Zarathustra from continuing. Twice they tell him to stop speaking, finally proposing a new lyre for new songs. The voices now show the conversation itself trying to change the speaker’s condition."
        },
        "question": {
          "ru": "Почему ответом на рассказ о болезни становится смена способа речи?",
          "en": "Why does changing the manner of speech answer an illness account?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "§2: конец рассказа об отвращении, два прерывания зверей и новая лира.",
              "en": "§2: the disgust account’s end, the animals’ two interruptions and the new lyre."
            }
          }
        ]
      },
      {
        "nodeId": "demo:character-author",
        "title": {
          "ru": "Речь внутри речи",
          "en": "Speech within speech"
        },
        "body": {
          "ru": "Звери называют Заратустру учителем возвращения, объясняют великий год и затем воображают то, что он сказал бы, если бы пожелал умереть теперь. Формула той же жизни входит в эту вложенную речь. В конце герой разговаривает со своей душой и не замечает, что звери умолкли; прочтению предстоит объяснить всю рамку.",
          "en": "The animals name Zarathustra recurrence’s teacher, explain the great year and then imagine what he would say if he wished to die now. The same-life formula belongs inside that speech. At the end he converses with his soul and does not notice their silence; a reading must explain the entire frame."
        },
        "question": {
          "ru": "Как вложенная речь и заключительное молчание влияют на её авторскую атрибуцию?",
          "en": "How do the embedded speech and closing silence affect authorial attribution?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Конец §2: предназначение, великий год, воображаемая последняя речь и молчание.",
              "en": "End of §2: vocation, great year, imagined final speech and silence."
            }
          }
        ]
      },
      {
        "nodeId": "demo:task",
        "title": {
          "ru": "Названная судьба учителя",
          "en": "The teacher’s named fate"
        },
        "body": {
          "ru": "Звери связывают учительство с величайшей опасностью и болезнью; новые песни должны помочь нести эту судьбу. В «Знамении» Заратустра сам называет своё дело, после того как отбрасывает соблазн жалости к высшим людям. Сопоставьте задачу, названную другими, с собственным наступившим часом.",
          "en": "The animals connect teaching with the greatest danger and illness; new songs are to help bear this fate. In “The Sign,” Zarathustra names his own work after passing the temptation of pity for the higher men. Compare a task named by others with one’s own arriving hour."
        },
        "question": {
          "ru": "Что нужно прочитать между называнием предназначения и словами о собственном деле?",
          "en": "What needs to be read between naming a vocation and speaking of one’s own work?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "§2: новая лира, предназначение учителя, опасность и болезнь.",
              "en": "§2: the new lyre, the teacher’s vocation, danger and illness."
            }
          },
          {
            "ref": "z-sign",
            "focus": {
              "ru": "От воспоминания об испытании жалостью к словам о своём деле.",
              "en": "From recalling pity’s temptation to the words about his own work."
            }
          }
        ]
      },
      {
        "nodeId": "demo:teach-live",
        "title": {
          "ru": "Нести мысль и выйти",
          "en": "Bear the thought and depart"
        },
        "body": {
          "ru": "Выздоравливающему предлагают новые песни, чтобы он мог нести свою судьбу; в финале «Знамения» Заратустра выходит из пещеры. Сравнение делает учительство движением между состоянием говорящего, способом речи и готовностью к делу. Вернитесь к первоначальной песне зверей и спросите, что она уже совершает для этого движения.",
          "en": "The convalescent is offered new songs so he may bear his fate; at “The Sign”’s end, Zarathustra leaves the cave. Comparison makes teaching a movement between the speaker’s condition, speech’s form and readiness for work. Return to the animals’ first song and ask what it already does for that movement."
        },
        "question": {
          "ru": "Что значит передавать мысль, которая ещё изменяет самого учителя?",
          "en": "What does it mean to convey a thought still changing the teacher?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "§2: пение как выздоровление и способность нести судьбу.",
              "en": "§2: singing as recovery and the capacity to bear fate."
            }
          },
          {
            "ref": "z-sign",
            "focus": {
              "ru": "Финальный выход Заратустры к своему делу.",
              "en": "Zarathustra’s final departure toward his work."
            }
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
          "ru": "Последовательность доходит до воображаемой речи самого Заратустры.",
          "en": "The sequence reaches the imagined speech of Zarathustra himself."
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
        "focus": {
          "ru": "§2 целиком: радость речи, песня, отвращение, новая лира и вложенная речь.",
          "en": "§2 in full: speech’s joy, song, disgust, new lyre and embedded speech."
        }
      },
      {
        "ref": "z-sign",
        "focus": {
          "ru": "Собственное дело и финальный выход.",
          "en": "His own work and the final departure."
        }
      }
    ]
  },
  {
    "id": "image-experience",
    "title": {
      "ru": "Образ и переживание",
      "en": "Image and experience"
    },
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
        "title": {
          "ru": "Сначала увидеть удушье",
          "en": "First see the choking"
        },
        "body": {
          "ru": "Пастух появляется после воя собаки и детского воспоминания. Заратустра видит судороги, искажённое лицо и змею в глотке; затем пытается помочь рукой. Прочитайте эту последовательность до всякого названия символа, чтобы сохранить задачу действия внутри видения.",
          "en": "The shepherd appears after the dog’s howl and a childhood recollection. Zarathustra sees convulsions, a distorted face and a snake in the throat, then tries to help with his hand. Read this sequence before naming a symbol, preserving the task of action within the vision."
        },
        "question": {
          "ru": "Что именно видит рассказчик прежде, чем пытается помочь?",
          "en": "What does the narrator see before he tries to help?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: появление пастуха и попытка вытянуть змею.",
              "en": "§2: the shepherd’s appearance and the attempt to pull out the snake."
            }
          }
        ]
      },
      {
        "nodeId": "demo:bite",
        "title": {
          "ru": "Рука и крик",
          "en": "The hand and the cry"
        },
        "body": {
          "ru": "Рука не справляется; из Заратустры вырывается приказ укусить. В крике сходятся ужас, ненависть, отвращение, жалость и всё доброе и злое в нём. Собственный укус пастуха отвечает на этот сложный призыв, меняя распределение действия.",
          "en": "The hand fails; a command to bite bursts from Zarathustra. Horror, hatred, disgust, pity and everything good and evil in him converge in the cry. The shepherd’s own bite answers this complex exhortation and redistributes the act."
        },
        "question": {
          "ru": "Почему действие пастуха нуждается в призыве, который не сводится к одному чувству?",
          "en": "Why does the shepherd’s act need an exhortation irreducible to one feeling?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: бессильная рука, состав крика и укус пастуха.",
              "en": "§2: the powerless hand, the cry’s gathered affects and the shepherd’s bite."
            }
          }
        ]
      },
      {
        "nodeId": "demo:laughter",
        "title": {
          "ru": "Смех, которого ещё не слышали",
          "en": "A laughter never heard before"
        },
        "body": {
          "ru": "После укуса повествование меняет имя увиденного: уже не пастух, не человек, а преображённый и смеющийся. Заратустра хочет вновь услышать этот смех и спрашивает, как теперь жить и умереть. Видение изменило и того, кто его рассказывает.",
          "en": "After the bite, the narrative renames what is seen: no longer shepherd or human, but transformed and laughing. Zarathustra wants to hear the laughter again and asks how to live and die now. The vision has changed its narrator too."
        },
        "question": {
          "ru": "Как желание рассказчика становится частью смысла преображения?",
          "en": "How does the narrator’s desire become part of transformation’s meaning?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "Финал §2: смена именования, смех и последние вопросы Заратустры.",
              "en": "End of §2: changed naming, laughter and Zarathustra’s final questions."
            }
          }
        ]
      },
      {
        "nodeId": "demo:affirmation",
        "title": {
          "ru": "Хотеть снова услышанного",
          "en": "Wanting what was heard again"
        },
        "body": {
          "ru": "Желание смеха можно поставить рядом с огромным мгновением §341, после которого слушающий ответил бы демону как богу. В обоих местах один опыт меняет вопрос об отношении к жизни. Сравнение требует уточнить, чего именно хотят вновь: услышанного смеха или всей жизни в прежнем порядке.",
          "en": "The desire for laughter can stand beside §341’s immense moment, after which the hearer might address the demon as a god. In both passages, one experience changes the relation to life. Comparison asks what is wanted again: the laughter heard or the entire life in its previous order."
        },
        "question": {
          "ru": "Как от желания одного переживания перейти к желанию возвращения жизни?",
          "en": "How can desire for one experience move toward desiring a life’s recurrence?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "Заключительное желание снова услышать смех.",
              "en": "The closing desire to hear the laughter again."
            }
          },
          {
            "ref": "gs341",
            "focus": {
              "ru": "Огромное мгновение и ответ демону; затем вопрос о всей жизни.",
              "en": "The immense moment and response to the demon; then the question of the whole life."
            }
          }
        ]
      },
      {
        "nodeId": "demo:performative",
        "title": {
          "ru": "Как желание произносится",
          "en": "How desire is voiced"
        },
        "body": {
          "ru": "В §341 мысль должна преобразить слушающего и сопровождать каждое действие. В «Семи печатях» желание вечности совершается через возвращающийся припев после разных условных строф. Переход позволяет исследовать, как утверждение получает форму, в которой его можно снова произнести.",
          "en": "In §341, the thought is to transform the hearer and accompany every act. In “The Seven Seals,” desire for eternity is enacted through a refrain returning after different conditional stanzas. The transition examines how affirmation receives a form in which it can be voiced again."
        },
        "question": {
          "ru": "Что повтор припева добавляет к одному признанию любви к вечности?",
          "en": "What does repeating the refrain add to a single avowal of love for eternity?"
        },
        "grounds": [
          {
            "ref": "gs341",
            "focus": {
              "ru": "Преобразующая мысль и вопрос при действии.",
              "en": "The transforming thought and the question accompanying action."
            }
          },
          {
            "ref": "z-seven-seals",
            "focus": {
              "ru": "Семь условных строф и общий припев.",
              "en": "The seven conditional stanzas and their common refrain."
            }
          }
        ]
      },
      {
        "nodeId": "demo:dance",
        "title": {
          "ru": "Тяжёлое делается лёгким",
          "en": "Heaviness becomes light"
        },
        "body": {
          "ru": "Шестая печать называет добродетелью танцора переход тяжёлого в лёгкое; тело получает образ духа-птицы. В седьмой говорящий хочет петь вместо речи словами. Проследите, как образ движения меняет самый способ обращения к вечности.",
          "en": "The sixth seal calls making heaviness light the dancer’s virtue; the body receives the bird-spirit’s image. In the seventh, the speaker wants singing in place of speech with words. Follow how movement’s image changes the manner of addressing eternity."
        },
        "question": {
          "ru": "Почему танец продолжается в переходе к пению?",
          "en": "Why does dance continue through the move to singing?"
        },
        "grounds": [
          {
            "ref": "z-seven-seals",
            "focus": {
              "ru": "§§6–7: танцор, дух-птица, слова и пение.",
              "en": "§§6–7: dancer, bird-spirit, words and singing."
            }
          }
        ]
      },
      {
        "nodeId": "demo:lived-rhythm",
        "title": {
          "ru": "Тот же припев после танца",
          "en": "The same refrain after the dance"
        },
        "body": {
          "ru": "Вернитесь к припеву после шестой печати и сравните его с первым появлением. Слова о вечности сохранены, но к ним привела другая образная дорога. Ритм текста позволяет услышать, как повторение удерживает желание и вместе с тем меняет его окружение.",
          "en": "Return to the refrain after the sixth seal and compare its first appearance. The words about eternity remain, but another path of images has led to them. The text’s rhythm makes repetition audible as retaining desire while changing its surroundings."
        },
        "question": {
          "ru": "Что в знакомом припеве слышится благодаря предшествующему танцу?",
          "en": "What becomes audible in the familiar refrain through the preceding dance?"
        },
        "grounds": [
          {
            "ref": "z-seven-seals",
            "focus": {
              "ru": "Припевы после §1 и §6 вместе с предшествующими строфами.",
              "en": "The refrains after §1 and §6 with their preceding stanzas."
            }
          }
        ]
      },
      {
        "nodeId": "demo:return-practice",
        "title": {
          "ru": "Вернуться с изменившимся слухом",
          "en": "Return with changed hearing"
        },
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
            "focus": {
              "ru": "Повтор выбранных строф и общего припева.",
              "en": "Rereading the chosen stanzas and their shared refrain."
            }
          },
          {
            "ref": "z-vision",
            "focus": {
              "ru": "Финальное желание Заратустры вновь услышать смех.",
              "en": "Zarathustra’s final longing to hear the laughter again."
            }
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
          "ru": "Собственный укус открывает смену имени и преображённый смех.",
          "en": "The shepherd’s own bite opens changed naming and transformed laughter."
        }
      },
      {
        "from": "demo:laughter",
        "to": "demo:affirmation",
        "edgeId": "atlas:laughter:interprets:affirmation",
        "body": {
          "ru": "Смех становится желанием рассказчика и вопросом о жизни.",
          "en": "Laughter becomes the narrator’s desire and a question about life."
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
          "ru": "Образ танцора вновь приводит к тому же припеву.",
          "en": "The dancer’s image leads back to the same refrain."
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
      "en": "The vision connected help, the shepherd’s own bite and the narrator’s longing; the song gave desire a returning form. Comparing laughter with refrain sharpened how an image can carry thought through changed hearing and expectation. Further reading can begin here: what does it mean to want again what once transformed desire?"
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
        "focus": {
          "ru": "Видение пастуха: помощь, крик, укус, смех и желание рассказчика.",
          "en": "The shepherd’s vision: help, cry, bite, laughter and the narrator’s longing."
        }
      },
      {
        "ref": "gs341",
        "focus": {
          "ru": "Огромное мгновение и желание повторения.",
          "en": "The immense moment and desire for recurrence."
        }
      },
      {
        "ref": "z-seven-seals",
        "focus": {
          "ru": "Припев и §§6–7 о танце и пении.",
          "en": "The refrain and §§6–7 on dance and singing."
        }
      }
    ]
  },
  {
    "id": "memory-others",
    "title": {
      "ru": "Память и другие",
      "en": "Memory and others"
    },
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
        "title": {
          "ru": "Память соединяет путь",
          "en": "Memory connects a path"
        },
        "body": {
          "ru": "Камю в финале эссе связывает последовательность поступков взглядом памяти; человек узнаёт в ней личную судьбу. В «О видении и загадке» память входит иначе: нынешний вой собаки вызывает детскую сцену. Сравните обозрение пути с одним узнаванием, которое ещё поведёт рассказ дальше.",
          "en": "At the essay’s close, Camus joins a sequence of actions through memory’s gaze; a person recognises a personal fate in it. In “The Vision and the Enigma,” memory enters differently: the present howl recalls a childhood scene. Compare surveying a path with one recognition that will carry the narrative onward."
        },
        "question": {
          "ru": "Что соединяется памятью в каждом из двух мест?",
          "en": "What does memory connect in each passage?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "focus": {
              "ru": "Финальные абзацы: взгляд памяти и соединённые поступки.",
              "en": "Final paragraphs: memory’s gaze and connected actions."
            }
          },
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: вой собаки и детское воспоминание.",
              "en": "§2: the dog’s howl and childhood recollection."
            }
          }
        ]
      },
      {
        "nodeId": "demo:remembered-return",
        "title": {
          "ru": "Узнать знакомый вой",
          "en": "Recognise a familiar howl"
        },
        "body": {
          "ru": "Заратустра слышал подобный вой в детстве и вспоминает собаку и луну. Настоящая сцена обретает узнаваемую деталь прежде, чем рассказчик понимает её нынешний повод. Возвращение здесь начинается со звука, соединяющего разные положения самого слушающего.",
          "en": "Zarathustra heard such a howl in childhood and remembers a dog and the moon. The present scene acquires a recognised detail before the narrator understands its present occasion. Return begins with a sound connecting different positions of the hearer himself."
        },
        "question": {
          "ru": "Почему узнавание звука ещё не даёт понимания нынешней сцены?",
          "en": "Why does recognising the sound not yet explain the present scene?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: прежняя собака и луна внутри нынешнего слышания.",
              "en": "§2: the earlier dog and moon within the present hearing."
            }
          }
        ]
      },
      {
        "nodeId": "demo:attention",
        "title": {
          "ru": "Следовать вопросу собаки",
          "en": "Follow the dog’s question"
        },
        "body": {
          "ru": "Воспоминание сменяется вопросом о том, что увидела собака; затем Заратустра видит невиданное прежде. Знакомое направляет внимание к новому предмету. Проследите этот переход, сохранив различие между воспоминанием рассказчика и ужасом собаки.",
          "en": "Recollection gives way to what the dog has seen; Zarathustra then sees something unprecedented. The familiar directs attention toward a new object. Follow the transition while distinguishing the narrator’s memory from the dog’s terror."
        },
        "question": {
          "ru": "Какое новое внимание рождается из узнавания?",
          "en": "What new attention arises from recognition?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: от детского воспоминания к вопросу о собаке и появлению пастуха.",
              "en": "§2: from childhood recollection to the dog’s question and the shepherd’s appearance."
            }
          }
        ]
      },
      {
        "nodeId": "demo:shared-world",
        "title": {
          "ru": "Разные участники одной сцены",
          "en": "Different participants in one scene"
        },
        "body": {
          "ru": "Собака воет, Заратустра тянет змею и кричит, пастух сам кусает. Ни один из этих ходов отдельно не описывает всю сцену. Общий мир видения можно прочитать как связь действий, в которой участники имеют разные места.",
          "en": "The dog howls, Zarathustra pulls the snake and cries out, the shepherd bites for himself. No one act describes the whole scene. The vision’s shared world can be read as connected actions in which participants occupy different positions."
        },
        "question": {
          "ru": "Что произошло бы с описанием сцены, если оставить только действие рассказчика?",
          "en": "What would happen to the scene’s description if only the narrator’s action remained?"
        },
        "grounds": [
          {
            "ref": "z-vision",
            "focus": {
              "ru": "§2: вой, попытка помощи, крик и собственный укус пастуха.",
              "en": "§2: the howl, attempted help, cry and the shepherd’s own bite."
            }
          }
        ]
      },
      {
        "nodeId": "demo:whole-part",
        "title": {
          "ru": "Целое с этими спутниками",
          "en": "A whole with these companions"
        },
        "body": {
          "ru": "В «Выздоравливающем» звери включают солнце, землю, орла и змею в воображаемую речь Заратустры о возвращении. Тождество жизни уточняется через именно это окружение. После видения пастуха вопрос становится конкретнее: какие отношения делают целое этой жизнью?",
          "en": "In “The Convalescent,” the animals include sun, earth, eagle and snake in Zarathustra’s imagined speech about return. Life’s identity is specified through these surroundings. After the shepherd’s vision, the question becomes more concrete: which relations make the whole this life?"
        },
        "question": {
          "ru": "Почему перечисление спутников предшествует словам о той же жизни?",
          "en": "Why are the companions listed before the same-life formula?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Конец §2: эта земля, солнце, орёл и змея в воображаемой речи.",
              "en": "End of §2: this earth, sun, eagle and snake in the imagined speech."
            }
          }
        ]
      },
      {
        "nodeId": "same-life",
        "title": {
          "ru": "Та же жизнь в большом и малом",
          "en": "The same life in large and small details"
        },
        "body": {
          "ru": "Звери воображают то, что Заратустра сказал бы, если бы пожелал умереть теперь: возвращение к той же жизни ради нового возвещения учения. Новая, лучшая и лишь похожая жизнь исключены. Целое включает и повторяемую задачу говорящего, и подробности его мира.",
          "en": "The animals imagine what Zarathustra would say if he wished to die now: returning to the same life to proclaim the teaching again. A new, better or merely similar life is excluded. The whole includes both the speaker’s recurring task and his world’s particulars."
        },
        "question": {
          "ru": "Как повтор задачи входит в тождество жизни?",
          "en": "How does the task’s recurrence enter life’s identity?"
        },
        "grounds": [
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Конец §2: условная речь о той же жизни и повторном учительстве.",
              "en": "End of §2: the conditional speech about the same life and teaching again."
            }
          }
        ]
      },
      {
        "nodeId": "demo:existential",
        "title": {
          "ru": "Хотеть всего хода",
          "en": "Wanting the entire course"
        },
        "body": {
          "ru": "Демон в §341 доводит полноту до каждой мысли, боли, радости, паука и самого разговора. Затем афоризм спрашивает о желании слушающего и его отношении к себе и жизни. Память привела маршрут к узнаванию, но испытание теперь обращено ко всему составу и порядку жизни.",
          "en": "The demon in §341 extends completeness to every thought, pain, joy, spider and the conversation itself. The aphorism then asks about the hearer’s desire and relation to self and life. Memory brought the route to recognition, but the trial now concerns life’s full contents and order."
        },
        "question": {
          "ru": "Что требуется от желания всей жизни сверх узнавания её отдельных возвращений?",
          "en": "What does wanting an entire life require beyond recognising particular returns?"
        },
        "grounds": [
          {
            "ref": "gs341",
            "focus": {
              "ru": "Речь демона и заключительный вопрос об отношении к себе и жизни.",
              "en": "The demon’s speech and the closing question of one’s relation to self and life."
            }
          }
        ]
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
          "ru": "Узнанный вой вызывает вопрос о том, что увидено теперь.",
          "en": "The recognised howl prompts what has now been seen."
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
          "ru": "Перечисление спутников ведёт к строгой формуле той же жизни.",
          "en": "Naming the companions leads to the strict same-life formula."
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
      "en": "Memory in Camus joins actions into fate; in Zarathustra’s vision, recognition opens a new object of attention. The shepherd’s scene and the imagined life’s companions show a whole formed through relations. Section 341 continues the reading: what does wanting a life’s entire course mean when recognition supplied only one entrance into it?"
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
        "focus": {
          "ru": "Память и связность поступков.",
          "en": "Memory and actions’ connectedness."
        }
      },
      {
        "ref": "z-vision",
        "focus": {
          "ru": "Детское воспоминание, внимание и участники видения.",
          "en": "Childhood recollection, attention and the vision’s participants."
        }
      },
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Спутники и та же жизнь в условной речи зверей.",
          "en": "Companions and the same life in the animals’ conditional speech."
        }
      },
      {
        "ref": "gs341",
        "focus": {
          "ru": "Полный состав и порядок жизни как предмет желания.",
          "en": "Life’s full contents and order as desire’s object."
        }
      }
    ]
  },
  {
    "id": "thinkers-necessity",
    "title": {
      "ru": "Необходимость у разных мыслителей",
      "en": "Necessity across thinkers"
    },
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
        "title": {
          "ru": "Начать с двух перечней",
          "en": "Begin with two lists"
        },
        "body": {
          "ru": "В начале «Энхиридиона» суждение, стремление, желание и отвращение названы подвластными; тело, имущество, слава и должности — неподвластными. Эпиктет сразу описывает, что происходит при смешении этих сторон. Вопрос свободы получает определённый предмет через то, что человек считает своим.",
          "en": "At the “Enchiridion”’s opening, judgement, impulse, desire and aversion are up to us; body, property, reputation and office are not. Epictetus immediately describes the consequences of confusing these sides. Freedom gains a definite object through what a person takes as their own."
        },
        "question": {
          "ru": "Почему ошибочное присвоение внешнего становится внутренней помехой?",
          "en": "Why does wrongly taking the external as one’s own become an inward obstruction?"
        },
        "grounds": [
          {
            "ref": "epictetus1",
            "focus": {
              "ru": "§1: перечни подвластного и неподвластного и последствия их смешения.",
              "en": "§1: the lists of what is and is not up to us and the consequences of confusing them."
            }
          }
        ]
      },
      {
        "nodeId": "demo:agency",
        "title": {
          "ru": "Участие из собственного действия",
          "en": "Participation through one’s own activity"
        },
        "body": {
          "ru": "Эпиктет выделяет наши собственные действия через суждения и стремления. В конце «Выздоравливающего» звери воображают речь, где Заратустра включён в причины своего возвращения. Сопоставьте область собственного действия с принадлежностью к причинной связи: это разные способы назвать участие.",
          "en": "Epictetus specifies our own activities through judgements and impulses. At “The Convalescent”’s end, the animals imagine a speech including Zarathustra among his return’s causes. Compare one’s own activity with membership in a causal nexus: these name participation differently."
        },
        "question": {
          "ru": "Совпадает ли «своё» с тем, чему человек служит причиной?",
          "en": "Does one’s own coincide with what a person causes?"
        },
        "grounds": [
          {
            "ref": "epictetus1",
            "focus": {
              "ru": "§1: наши собственные действия.",
              "en": "§1: our own activities."
            }
          },
          {
            "ref": "z-convalescent",
            "focus": {
              "ru": "Конец §2: звери воображают Заратустру среди причин его возвращения.",
              "en": "End of §2: the animals imagine Zarathustra among the causes of his return."
            }
          }
        ]
      },
      {
        "nodeId": "demo:fate",
        "title": {
          "ru": "Как судьба становится своей",
          "en": "How fate becomes one’s own"
        },
        "body": {
          "ru": "У Камю поступки соединяются взглядом памяти в личную судьбу. На «Блаженных островах» Заратустра говорит о воле, которая хочет такой судьбы. Принадлежность пути себе получает различие между сделанным, обозреваемым и желаемым.",
          "en": "For Camus, memory’s gaze joins actions into personal fate. On “The Happy Isles,” Zarathustra speaks of a will wanting such a fate. One’s path belonging to oneself differs across what is done, surveyed and wanted."
        },
        "question": {
          "ru": "Как желание судьбы соотносится с узнаванием сделанного своим?",
          "en": "How does wanting fate relate to recognising what was done as one’s own?"
        },
        "grounds": [
          {
            "ref": "camus-sisyphus",
            "focus": {
              "ru": "Финал: поступки, память и личная судьба.",
              "en": "Ending: actions, memory and personal fate."
            }
          },
          {
            "ref": "z-happy-isles",
            "focus": {
              "ru": "Поправка о воле, которая хочет такой судьбы.",
              "en": "The correction about the will wanting such a fate."
            }
          }
        ]
      },
      {
        "nodeId": "demo:free-necessary",
        "title": {
          "ru": "Поправка воли",
          "en": "Willing’s correction"
        },
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
            "focus": {
              "ru": "Творчество, освобождающее хотение и две формулировки воли и судьбы.",
              "en": "Creation, liberating willing and the two formulations of will and fate."
            }
          }
        ]
      },
      {
        "nodeId": "demo:necessity",
        "title": {
          "ru": "Источник определения",
          "en": "Determination’s source"
        },
        "body": {
          "ru": "Спиноза в определении 7 первой части различает определение собственной природой и другим. Необходимость собственной природы входит в свободное существование и действие. Вопрос теперь касается того, откуда действует вещь, а не одного присутствия необходимости.",
          "en": "In Part I, definition 7, Spinoza distinguishes determination by one’s own nature from determination by another. One’s own nature’s necessity enters free existence and action. The question now concerns where a thing acts from, beyond necessity’s mere presence."
        },
        "question": {
          "ru": "Что нужно знать о природе вещи, чтобы назвать её действие свободным?",
          "en": "What must be known about a thing’s nature to call its action free?"
        },
        "grounds": [
          {
            "ref": "spinoza1d7",
            "focus": {
              "ru": "I, определение 7: существование и действие из собственной природы или по определению другого.",
              "en": "I, definition 7: existence and action from one’s own nature or by another’s determination."
            }
          }
        ]
      },
      {
        "nodeId": "demo:spinoza",
        "title": {
          "ru": "Собственная сила вещи",
          "en": "A thing’s own power"
        },
        "body": {
          "ru": "Поставьте определение свободы рядом с III.6: каждая вещь, насколько от неё зависит, стремится сохранять бытие. Доказательство говорит о её действующей силе и противостоянии уничтожению. Это сравнение уточняет собственное начало действия, оставляя вопрос о том, как вещь соотносится с другими причинами.",
          "en": "Place freedom’s definition beside III.6: each thing, as far as it can by its own power, strives to persevere in being. The proof speaks of its active power and opposition to destruction. The comparison specifies action’s own source while leaving how a thing relates to other causes in question."
        },
        "question": {
          "ru": "Как «насколько от неё зависит» уточняет собственную силу вещи?",
          "en": "How does “as far as it can by its own power” specify a thing’s power?"
        },
        "grounds": [
          {
            "ref": "spinoza1d7",
            "focus": {
              "ru": "Определение свободы через собственную природу.",
              "en": "Freedom defined through one’s own nature."
            }
          },
          {
            "ref": "spinoza3p6",
            "focus": {
              "ru": "III.6 и доказательство: собственная сила и сохранение бытия.",
              "en": "III.6 and proof: one’s own power and perseverance in being."
            }
          }
        ]
      },
      {
        "nodeId": "demo:leibniz",
        "title": {
          "ru": "Основание выбора вселенной",
          "en": "A reason for choosing a universe"
        },
        "body": {
          "ru": "Лейбниц в §§53–55 начинает с множества возможных вселенных и существования одной. Вопрос требует основания выбора, которое связывается со степенью совершенства. Рядом со Спинозой сравните работу основания: определять действие из природы или объяснять выбор существующего целого.",
          "en": "In §§53–55, Leibniz begins with many possible universes and the existence of one. The question demands a reason for selection, connected with degree of perfection. Beside Spinoza, compare grounding’s work: determining action from nature or explaining an existing whole’s selection."
        },
        "question": {
          "ru": "Почему возможность многих миров заставляет спрашивать об основании этого?",
          "en": "Why do many possible worlds prompt the question of this one’s reason?"
        },
        "grounds": [
          {
            "ref": "leibniz53-55",
            "focus": {
              "ru": "§§53–55: множество вселенных, выбор и совершенство.",
              "en": "§§53–55: many universes, selection and perfection."
            }
          },
          {
            "ref": "spinoza1d7",
            "focus": {
              "ru": "Определение 7: действие из собственной природы.",
              "en": "Definition 7: action from one’s own nature."
            }
          }
        ]
      },
      {
        "nodeId": "demo:possible-worlds",
        "title": {
          "ru": "Возможное внутри объяснения",
          "en": "Possibility within the explanation"
        },
        "body": {
          "ru": "Возможные вселенные у Лейбница различаются совершенством; божественные мудрость, благость и могущество участвуют в существовании избранного мира. Возможность здесь принадлежит объяснению целого. Вернитесь с этим масштабом к Эпиктету: его вопрос о подвластном обращён к иной области действия.",
          "en": "Leibniz’s possible universes differ in perfection; divine wisdom, goodness and power enter the chosen world’s existence. Possibility belongs here to an explanation of the whole. Return with this scale to Epictetus: his question of what is up to us addresses another field of activity."
        },
        "question": {
          "ru": "Как меняется смысл выбора при переходе от собственного суждения к возможной вселенной?",
          "en": "How does choosing change from one’s own judgement to a possible universe?"
        },
        "grounds": [
          {
            "ref": "leibniz53-55",
            "focus": {
              "ru": "§§54–55: совершенство и божественный выбор.",
              "en": "§§54–55: perfection and divine selection."
            }
          },
          {
            "ref": "epictetus1",
            "focus": {
              "ru": "§1: собственные суждения и стремления.",
              "en": "§1: one’s own judgements and impulses."
            }
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
        "body": {
          "ru": "Выбор вселенной требует различить возможности внутри лейбницевского объяснения.",
          "en": "Selecting a universe requires distinguishing possibilities within Leibniz’s explanation."
        }
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
        "focus": {
          "ru": "Подвластное и собственная деятельность.",
          "en": "What is up to us and one’s own activity."
        }
      },
      {
        "ref": "z-convalescent",
        "focus": {
          "ru": "Участник среди причин в условной речи зверей.",
          "en": "The participant among causes in the animals’ conditional speech."
        }
      },
      {
        "ref": "camus-sisyphus",
        "focus": {
          "ru": "Поступки и личная судьба.",
          "en": "Actions and personal fate."
        }
      },
      {
        "ref": "z-happy-isles",
        "focus": {
          "ru": "Воля хочет такой судьбы.",
          "en": "The will wants such a fate."
        }
      },
      {
        "ref": "spinoza1d7",
        "focus": {
          "ru": "Необходимость собственной природы и определение другим.",
          "en": "One’s own nature’s necessity and determination by another."
        }
      },
      {
        "ref": "spinoza3p6",
        "focus": {
          "ru": "Собственная сила вещи в сохранении бытия.",
          "en": "A thing’s own power in persevering in being."
        }
      },
      {
        "ref": "leibniz53-55",
        "focus": {
          "ru": "Возможные вселенные и основание выбора.",
          "en": "Possible universes and selection’s reason."
        }
      }
    ]
  }
];
