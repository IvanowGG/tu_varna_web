//! Built-in document templates, used when no AI key is configured. They follow common
//! Bulgarian university conventions and leave [placeholders] for the student's own data.

use crate::ai::GenReq;

fn title(r: &GenReq, bg: bool) -> String {
    let t = r.topic.trim();
    if t.is_empty() {
        if bg {
            "Без заглавие".into()
        } else {
            "Untitled".into()
        }
    } else {
        t.to_string()
    }
}

fn cover(r: &GenReq, bg: bool, kind_label: &str) -> String {
    if bg {
        format!(
            "**Технически университет – Варна**\n\n**{kind_label}**\n\n# {}\n\n\
Изготвил: [име, фамилия], фак. № [фак. №], специалност [специалност], курс [курс], група [група]\n\n\
Проверил: [титла, име на преподавателя]\n\nВарна, [година]\n\n---\n\n",
            title(r, bg)
        )
    } else {
        format!(
            "**Technical University of Varna**\n\n**{kind_label}**\n\n# {}\n\n\
Prepared by: [name], faculty no. [number], specialty [specialty], year [year], group [group]\n\n\
Supervisor: [title, lecturer's name]\n\nVarna, [year]\n\n---\n\n",
            title(r, bg)
        )
    }
}

pub fn template(r: &GenReq) -> String {
    let bg = r.language != "en";
    let notes = if r.details.trim().is_empty() {
        String::new()
    } else if bg {
        format!(
            "> Вашите бележки: {}\n\n",
            r.details.trim().replace('\n', " ")
        )
    } else {
        format!("> Your notes: {}\n\n", r.details.trim().replace('\n', " "))
    };
    let offline = if bg {
        "> Офлайн шаблон: AI асистентът не е включен на този сървър. Попълнете местата в [скоби].\n\n"
    } else {
        "> Offline template: the AI assistant is not enabled on this server. Fill in the [brackets].\n\n"
    };

    match r.action.as_str() {
        "improve" | "continue" | "translate" => {
            return format!("{offline}{}\n", r.text.trim());
        }
        "summarize" | "questions" => {
            // Pull the headings out of the text as a starting skeleton.
            let heads: Vec<&str> = r
                .text
                .lines()
                .filter(|l| l.trim_start().starts_with('#'))
                .collect();
            let mut s = String::from(offline);
            s.push_str(if bg {
                "# Конспект\n\n"
            } else {
                "# Summary\n\n"
            });
            if heads.is_empty() {
                s.push_str(if bg { "## Основни идеи\n- [идея 1]\n- [идея 2]\n\n## Определения\n- [термин] – [определение]\n\n## Въпроси за изпита\n1. [въпрос]\n" } else { "## Key ideas\n- [idea 1]\n- [idea 2]\n\n## Definitions\n- [term] – [definition]\n\n## Exam questions\n1. [question]\n" });
            } else {
                for h in heads {
                    let h = h.trim_start_matches('#').trim();
                    s.push_str(&format!(
                        "## {h}\n- [{}]\n\n",
                        if bg {
                            "основна мисъл"
                        } else {
                            "key point"
                        }
                    ));
                }
            }
            return s;
        }
        _ => {}
    }

    let body = match (r.kind.as_str(), bg) {
        ("essay", true) => format!(
            "# {}\n\n## Увод\n[Представете темата и защо е важна. Завършете с ясна теза.]\n\n\
## Изложение\n### [Първи аргумент]\n[Обяснение, пример, [източник].]\n\n### [Втори аргумент]\n[Обяснение, пример.]\n\n\
### [Контрааргумент и отговор]\n[Каква е другата гледна точка и защо тезата остава вярна.]\n\n\
## Заключение\n[Обобщете аргументите и върнете към тезата.]\n\n## Използвана литература\n1. [Автор, заглавие, година]\n",
            title(r, bg)
        ),
        ("essay", false) => format!(
            "# {}\n\n## Introduction\n[Introduce the topic and why it matters. End with a clear thesis.]\n\n\
## Body\n### [First argument]\n[Explanation, example, [source].]\n\n### [Second argument]\n[Explanation, example.]\n\n\
### [Counter-argument and reply]\n\n## Conclusion\n[Sum up and return to the thesis.]\n\n## References\n1. [Author, title, year]\n",
            title(r, bg)
        ),
        ("report", _) | ("coursework", _) => {
            let label = match (r.kind.as_str(), bg) {
                ("report", true) => "РЕФЕРАТ",
                ("report", false) => "REPORT",
                (_, true) => "КУРСОВА РАБОТА",
                (_, false) => "COURSEWORK",
            };
            if bg {
                format!(
                    "{}## Съдържание\n1. Увод\n2. Теоретична част\n3. Практическа част\n4. Резултати и анализ\n5. Заключение\n6. Използвана литература\n\n\
## 1. Увод\n[Цел, задачи и обхват на работата.]\n\n## 2. Теоретична част\n[Основни понятия и известни подходи, [източник].]\n\n\
## 3. Практическа част\n[Какво е направено: модел, програма, схема, експеримент.]\n\n## 4. Резултати и анализ\n[Таблици, графики и какво показват.]\n\n\
## 5. Заключение\n[Изпълнени ли са целите; възможни подобрения.]\n\n## 6. Използвана литература\n1. [Автор, заглавие, издателство, година]\n",
                    cover(r, bg, label)
                )
            } else {
                format!(
                    "{}## Contents\n1. Introduction\n2. Theory\n3. Practical part\n4. Results and analysis\n5. Conclusion\n6. References\n\n\
## 1. Introduction\n[Aim, tasks and scope.]\n\n## 2. Theory\n[Key concepts and known approaches, [source].]\n\n\
## 3. Practical part\n[What was built or measured.]\n\n## 4. Results and analysis\n[Tables, charts, what they show.]\n\n\
## 5. Conclusion\n[Were the aims met; improvements.]\n\n## 6. References\n1. [Author, title, publisher, year]\n",
                    cover(r, bg, label)
                )
            }
        }
        ("lab", true) => format!(
            "{}## 1. Цел на упражнението\n[Какво трябва да се изследва.]\n\n## 2. Теоретична постановка\n[Формули и зависимости.]\n\n\
## 3. Опитна постановка и апаратура\n- [уред, модел]\n- [уред, модел]\n\n## 4. Ход на работата\n1. [стъпка]\n2. [стъпка]\n\n\
## 5. Резултати\n| № | [величина] | [величина] |\n|---|---|---|\n| 1 | [измерена стойност] | [измерена стойност] |\n\n\
## 6. Изчисления\n[Формула → заместване → резултат с мерни единици.]\n\n## 7. Изводи\n[Какво показват резултатите; грешки и причини.]\n",
            cover(r, bg, "ПРОТОКОЛ ОТ ЛАБОРАТОРНО УПРАЖНЕНИЕ")
        ),
        ("lab", false) => format!(
            "{}## 1. Aim\n\n## 2. Theory\n[Formulas.]\n\n## 3. Equipment\n- [device, model]\n\n## 4. Procedure\n1. [step]\n\n\
## 5. Results\n| # | [quantity] | [quantity] |\n|---|---|---|\n| 1 | [value] | [value] |\n\n## 6. Calculations\n\n## 7. Conclusions\n",
            cover(r, bg, "LAB REPORT")
        ),
        ("email", true) => format!(
            "**Тема:** {}\n\nУважаеми/а [доц./проф./гл. ас.] [фамилия],\n\n[Едно изречение кой сте: име, специалност, курс, група, фак. №.]\n\n\
[Ясно и кратко: какво молите или питате и защо.]\n\n[Ако е нужно: какво предлагате – дата, час, файл в прикачен файл.]\n\n\
Благодаря Ви предварително!\n\nС уважение,\n[име, фамилия]\n[специалност, курс, група], фак. № [фак. №]\n",
            title(r, bg)
        ),
        ("email", false) => format!(
            "**Subject:** {}\n\nDear [title] [surname],\n\n[Who you are: name, specialty, year, group.]\n\n[Your request, short and clear.]\n\n\
Thank you in advance.\n\nKind regards,\n[name]\n[specialty, year, group]\n",
            title(r, bg)
        ),
        ("cv", true) => "# [Име Фамилия]\n[град] · [телефон] · [имейл] · [LinkedIn/GitHub]\n\n## Профил\n[2–3 изречения: кой сте и какво търсите.]\n\n\
## Образование\n**Технически университет – Варна**, [специалност] — [година]–[година]\n- [курсови проекти, успех]\n\n\
## Опит\n**[Позиция]**, [фирма] — [период]\n- [постижение с число]\n\n## Умения\n- [език/технология] — [ниво]\n\n## Езици\n- Английски — [ниво]\n"
            .to_string(),
        ("cv", false) => "# [Name Surname]\n[city] · [phone] · [e-mail] · [LinkedIn/GitHub]\n\n## Profile\n\n## Education\n**Technical University of Varna**, [specialty] — [years]\n\n\
## Experience\n**[Role]**, [company] — [period]\n- [achievement with a number]\n\n## Skills\n\n## Languages\n"
            .to_string(),
        ("presentation", true) => format!(
            "# {}\n\n## Слайд 1 — Заглавие\n- [тема, име, специалност]\n\n## Слайд 2 — Цел\n- [какво ще покажете]\n\n## Слайд 3 — Проблем\n- [факт]\n\n\
## Слайд 4–6 — Основна част\n- [по една идея на слайд]\n\n## Слайд 7 — Резултати\n- [графика / демо]\n\n## Слайд 8 — Изводи\n- [3 извода]\n\n## Слайд 9 — Въпроси\n",
            title(r, bg)
        ),
        ("presentation", false) => format!(
            "# {}\n\n## Slide 1 — Title\n## Slide 2 — Goal\n## Slide 3 — Problem\n## Slides 4–6 — Main part\n## Slide 7 — Results\n## Slide 8 — Conclusions\n## Slide 9 — Questions\n",
            title(r, bg)
        ),
        ("letter", true) => format!(
            "**ДО**\n**РЕКТОРА / ДЕКАНА НА [ФАКУЛТЕТ]**\n**НА ТУ – ВАРНА**\n\n# ЗАЯВЛЕНИЕ\n\nот [име, презиме, фамилия], фак. № [фак. №], специалност [специалност], курс [курс], форма на обучение [редовна/задочна]\n\n\
**Относно:** {}\n\nУважаеми господин Ректор / Декан,\n\n[Молбата Ви и основанието за нея.]\n\nПрилагам: [документи]\n\nДата: [дата]\nгр. Варна\n\nС уважение: ...................\n",
            title(r, bg)
        ),
        ("letter", false) => format!(
            "**TO THE RECTOR / DEAN OF [FACULTY]**\n**TECHNICAL UNIVERSITY OF VARNA**\n\n# APPLICATION\n\nfrom [full name], faculty no. [number], specialty [specialty]\n\n\
**Re:** {}\n\n[Your request and the reason.]\n\nAttached: [documents]\n\nDate: [date]\n\nSignature: ...................\n",
            title(r, bg)
        ),
        (_, true) => format!(
            "# {}\n\n## Основни понятия\n- [термин] – [определение]\n\n## Ключови идеи\n1. [идея]\n2. [идея]\n\n## Формули\n- [формула] — [какво означава всяка величина]\n\n## Примери\n- [пример]\n\n## Въпроси за самопроверка\n1. [въпрос]\n",
            title(r, bg)
        ),
        (_, false) => format!(
            "# {}\n\n## Key terms\n- [term] – [definition]\n\n## Key ideas\n1. [idea]\n\n## Formulas\n\n## Examples\n\n## Self-test questions\n1. [question]\n",
            title(r, bg)
        ),
    };
    let body = if r.action == "outline" {
        body.lines()
            .filter(|l| l.starts_with('#') || l.starts_with("**"))
            .collect::<Vec<_>>()
            .join("\n\n")
            + "\n"
    } else {
        body
    };
    format!("{offline}{notes}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(kind: &str, lang: &str) -> GenReq {
        GenReq {
            action: "write".into(),
            kind: kind.into(),
            topic: "Тест".into(),
            details: String::new(),
            language: lang.into(),
            length: "medium".into(),
            tone: "academic".into(),
            text: String::new(),
            include_profile: false,
        }
    }

    #[test]
    fn every_kind_has_a_template_in_both_languages() {
        for k in [
            "essay",
            "report",
            "coursework",
            "lab",
            "summary",
            "email",
            "cv",
            "presentation",
            "letter",
            "notes",
            "other",
        ] {
            for l in ["bg", "en"] {
                let t = template(&r(k, l));
                assert!(t.len() > 120, "{k}/{l} too short");
                assert!(t.contains('['), "{k}/{l} should leave placeholders");
            }
        }
        assert!(template(&r("lab", "bg")).contains("Технически университет – Варна"));
    }
}
