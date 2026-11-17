// Bulgarian (default) and English. t(key, ...args) fills {0}, {1}…

const BG = {
  app_name: 'Студентски хъб', app_sub: 'ТУ-Варна',
  nav_dashboard: 'Начало', nav_schedule: 'Разписание', nav_tasks: 'Задачи', nav_notes: 'Бележки', nav_grades: 'Успех',
  nav_docs: 'Документи', nav_focus: 'Фокус', nav_links: 'Връзки', nav_settings: 'Настройки', nav_profile: 'Профил', nav_more: 'Още',
  close: 'Затвори', cancel: 'Отказ', delete: 'Изтрий', save: 'Запази', add: 'Добави', edit: 'Редактирай', are_you_sure: 'Сигурни ли сте?',
  d_short: 'д', today: 'Днес', tomorrow: 'Утре', yesterday: 'Вчера', in_days: 'след {0} дни', days_ago: 'преди {0} дни',
  theme: 'Тема', theme_auto: 'Авто', theme_light: 'Светла', theme_dark: 'Тъмна', language: 'Език',
  sign_out: 'Изход', loading: 'Зареждане…', retry: 'Опитай пак', offline: 'Няма връзка — показваме запазеното.',

  // Landing
  hero_title_1: 'Цялото следване,', hero_title_2: 'на едно място.',
  hero_lead: 'Точното ти разписание от сайта на ТУ-Варна, задачи и срокове, бележки, успех и асистент, който пише документи вместо теб — влизаш с университетския си Microsoft акаунт.',
  sign_in_ms: 'Вход с Microsoft', try_demo: 'Пробвай демо', ms_not_configured: 'Входът с Microsoft още не е настроен на този сървър.',
  trust_local: 'Данните ти остават в браузъра', trust_no_pw: 'Не пазим пароли', trust_captcha: 'Защита от ботове',
  f_schedule: 'Точно разписание', f_schedule_d: 'Избираш специалност, курс и група — взимаме разписа директно от umo.tu-varna.bg, с подгрупи и четни/нечетни седмици.',
  f_docs: 'Асистент за документи', f_docs_d: 'Реферати, курсови, протоколи, имейли до преподаватели, CV — чернова за секунди, експорт в Word.',
  f_tasks: 'Задачи и изпити', f_tasks_d: 'Срокове, домашни и изпитна сесия с обратно броене и напомняне какво изтича.',
  f_grades: 'Успех', f_grades_d: 'Среден успех с кредити (ECTS) по шестобалната система, по семестри.',
  f_focus: 'Фокус таймер', f_focus_d: 'Pomodoro сесии и статистика колко си учил тази седмица.',
  f_private: 'Поверително', f_private_d: 'Всичко лично стои в браузъра ти. Сървърът пази само временна сесия, без пароли и токени на диск.',
  auth_error_title: 'Входът не успя', perm_error_title: 'Достъпът от Microsoft не беше даден',
  ae_captcha: 'Първо потвърдете, че не сте робот.', ae_session: 'Сесията изтече. Опитайте отново.', ae_expired: 'Опитът за вход изтече. Опитайте отново.',
  ae_cancelled: 'Входът беше отказан.', ae_consent: 'Университетът трябва да одобри приложението (admin consent). Свържете се с администратора.',
  ae_domain: 'Влезте с университетския си акаунт (@tu-varna.bg).', ae_not_configured: 'Входът с Microsoft не е настроен.', ae_rate_limited: 'Твърде много опити. Изчакайте минута.',
  ae_generic: 'Нещо се обърка при входа. Опитайте отново.',

  // Captcha
  cap_title: 'Потвърдете, че сте човек', cap_hint: 'Плъзнете синьото кръгче върху празния кръг.',
  cap_working: 'Проверяваме браузъра…', cap_ready: 'Готово — плъзнете кръгчето.', cap_checking: 'Проверка…', cap_ok: 'Потвърдено',
  cap_bad: 'Не стана. Ето нов пъзел.', cap_cross_site: 'Сървърът не приема заявки от този адрес. Отворете сайта на адреса, който сървърът показва при старт.', cap_keys: 'С клавиатура: ← → и Enter.', cap_new: 'Нов пъзел',

  // Dashboard
  good_morning: 'Добро утро', good_day: 'Добър ден', good_evening: 'Добър вечер',
  week_n: 'Седмица {0}', odd_week: 'нечетна', even_week: 'четна', before_semester: 'преди семестъра',
  today_classes: 'Днес', no_classes_today: 'Днес нямате занятия.', set_up_schedule: 'Настройте разписанието си', set_up: 'Настрой',
  next_class: 'Следващо занятие', starts_in: 'започва след', now_in: 'Сега', ends_in: 'свършва след', no_more_today: 'Няма повече занятия днес.',
  due_soon: 'Изтичат скоро', nothing_due: 'Нищо не изтича скоро.', overdue_n: '{0} просрочени', exams: 'Изпити', no_exams: 'Няма добавени изпити.',
  avg_grade: 'Среден успех', no_grades: 'Още няма оценки.', focus_today: 'Фокус днес', minutes_short: 'мин',
  quick_add: 'Бърза задача…', open: 'Отвори', more_n: '+ още {0} →', no_links: 'Още нямате свои връзки. Добавете първата отгоре.', mark_done: 'Отбележи като готово',

  // Schedule
  sch_setup: 'Твоята група', semester: 'Семестър', winter: 'Зимен', summer: 'Летен', specialty: 'Специалност', course: 'Курс', group: 'Група',
  load_schedule: 'Зареди разписанието', or_paste_url: 'или постави адрес на разписание от umo.tu-varna.bg', url: 'Адрес', load: 'Зареди',
  subgroup: 'Подгрупа', all: 'Всички', show: 'Покажи', this_week_only: 'Само тази седмица', every_week: 'Всяка седмица',
  updated: 'Обновено {0}', refresh: 'Обнови', source: 'Източник', change_group: 'Смени групата',
  k_lecture: 'Лекция', k_lab: 'Лабораторно', k_seminar: 'Семинарно', k_exercise: 'Упражнение', k_seminar_lab: 'Сем. + лаб.', k_project: 'Курсов проект', k_other: 'Друго',
  odd_only: 'нечетна седм.', even_only: 'четна седм.', days: ['Понеделник', 'Вторник', 'Сряда', 'Четвъртък', 'Петък', 'Събота', 'Неделя'],
  days_short: ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Нд'],
  sem_start_hint: 'Номерът на седмицата се смята от началото на семестъра ({0}). Ако не съвпада, сменете го в Настройки.',
  sch_notes: 'Бележки от разписа', no_lessons: 'В този разпис няма занятия.', week_prev: 'Предишна седмица', week_next: 'Следваща седмица', this_week: 'Тази седмица',
  ms_calendar: 'Календар от Microsoft', ms_calendar_none: 'Няма достъп до календара (университетът не използва Outlook за пощата).',
  err_host_not_allowed: 'Разрешени са само адреси от tu-varna.bg.', err_no_timetable_for_group: 'Няма разписание за тази комбинация.',
  err_upstream: 'Сайтът на университета не отговаря. Опитайте след малко.', err_parse: 'Файлът не прилича на разписание на ТУ-Варна.',

  // Tasks
  new_task: 'Нова задача', title: 'Заглавие', subject: 'Предмет', due: 'Срок', type: 'Вид', priority: 'Важност',
  t_task: 'Задача', t_homework: 'Домашно', t_exam: 'Изпит', t_project: 'Проект', p_low: 'Ниска', p_normal: 'Нормална', p_high: 'Висока',
  g_overdue: 'Просрочени', g_today: 'Днес', g_week: 'Тази седмица', g_later: 'По-късно', g_nodate: 'Без срок', g_done: 'Готови',
  no_tasks: 'Нямате задачи. Добавете първата!', filter_all: 'Всички', filter_open: 'Отворени', filter_exams: 'Изпити', clear_done: 'Изчисти готовите',

  // Notes
  new_note: 'Нова бележка', search: 'Търси…', untitled: 'Без заглавие', saved_locally: 'Запазено в браузъра', no_notes: 'Още нямате бележки.',
  preview: 'Преглед', write: 'Писане', pick_note: 'Изберете или създайте бележка.',

  // Grades
  add_subject: 'Добави предмет', credits: 'Кредити', grade: 'Оценка', weighted_avg: 'Среден успех (с кредити)', simple_avg: 'Обикновен среден',
  total_credits: 'Кредити', gr_6: 'Отличен', gr_5: 'Много добър', gr_4: 'Добър', gr_3: 'Среден', gr_2: 'Слаб', not_graded: '—',
  grades_hint: 'Шестобална система. Средният успех се тежи по кредитите (ECTS).',

  // Docs
  doc_assistant: 'Асистент за документи', what: 'Какво', action: 'Действие', topic: 'Тема', details: 'Изисквания и бележки', details_ph: 'Напр.: 10 страници, цел, какво да включва, източници, които имате…',
  length: 'Обем', short: 'Кратко', medium: 'Средно', long: 'Дълго', tone: 'Стил', academic: 'Академичен', neutral: 'Неутрален', friendly: 'Приятелски',
  include_profile: 'Добави името и специалността ми (за титулна страница)', generate: 'Създай', stop: 'Спри',
  a_write: 'Напиши', a_outline: 'План', a_improve: 'Подобри', a_continue: 'Продължи', a_summarize: 'Конспект', a_questions: 'Въпроси', a_translate: 'Преведи',
  d_essay: 'Есе', d_report: 'Реферат', d_coursework: 'Курсова работа', d_lab: 'Протокол (лабораторно)', d_summary: 'Конспект', d_email: 'Имейл до преподавател',
  d_cv: 'CV', d_presentation: 'Презентация', d_letter: 'Заявление', d_notes: 'Учебни бележки', d_other: 'Друго',
  my_docs: 'Моите документи', new_doc: 'Нов документ', export_docx: 'Word', export_md: 'Markdown', print: 'Печат / PDF', no_docs: 'Още няма документи.',
  ai_on: 'AI: {0}', ai_off: 'Офлайн шаблони (AI не е включен на сървъра)', thinking: 'Мисли…', writing: 'Пише…', template_mode: 'Шаблон',
  ai_errors_hourly_limit: 'Достигнахте лимита за час. Опитайте по-късно.', ai_errors_daily_limit: 'Дневният лимит на сайта е достигнат.',
  ai_errors_ai_busy: 'AI услугата е натоварена. Опитайте след малко.', ai_errors_ai_key: 'AI ключът на сървъра не е валиден.', ai_errors_refusal: 'Асистентът отказа тази заявка.',
  ai_errors_generic: 'Неуспешно генериране. Опитайте пак.', ai_errors_no_topic: 'Напишете тема.', ai_errors_no_text: 'Първо трябва да има текст в документа.',
  ai_note: 'Проверявайте фактите. Асистентът оставя [места] за вашите данни и източници.', words: '{0} думи',
  apply_to_doc: 'Действията отдолу работят върху текста в редактора.',

  // Focus
  focus: 'Фокус', break: 'Почивка', start: 'Старт', pause: 'Пауза', reset: 'Нулирай', skip: 'Прескочи', this_week_focus: 'Тази седмица', sessions_today: 'Сесии днес',
  focus_done: 'Сесията свърши — време за почивка!', break_done: 'Почивката свърши — продължаваме!',

  // Links
  uni_links: 'ТУ-Варна', my_links: 'Моите връзки', add_link: 'Добави връзка', name: 'Име',
  l_site: 'Официален сайт', l_umo: 'Учебни разписания (УМО)', l_elearning: 'еТУ-Варна (Moodle)', l_sis: 'Студентска информационна система',
  l_mail: 'Университетска поща', l_teams: 'Microsoft Teams', l_office: 'Microsoft 365',

  // Settings
  appearance: 'Изглед', semester_start: 'Начало на семестъра (седмица 1)', semester_start_hint: 'Понеделникът на първата учебна седмица. От него смятаме четна/нечетна седмица.',
  my_subgroup: 'Моята подгрупа', data: 'Данни', export_data: 'Изтегли копие (JSON)', import_data: 'Възстанови от копие', clear_data: 'Изтрий всичко на това устройство',
  data_hint: 'Задачите, бележките, оценките и документите се пазят само в този браузър. Изтеглете копие, ако сменяте устройство.',
  privacy: 'Поверителност', privacy_text: 'Сървърът не пише нищо на диск. Пази в паметта временна сесия (случаен идентификатор в бисквитка HttpOnly) и Microsoft токена за до един час. Паролата ви никога не минава през нас — въвеждате я само на страницата на Microsoft.',
  imported: 'Данните са възстановени.', import_failed: 'Файлът не е валидно копие.', cleared: 'Изтрито.',

  // Profile
  from_microsoft: 'От вашия Microsoft акаунт', email: 'Имейл', department: 'Звено / специалност', job_title: 'Длъжност', office: 'Офис',
  faculty_number: 'Номер (фак. №)', organization: 'Организация', permissions: 'Разрешения', demo_account: 'Демо акаунт',
};

const EN = {
  app_name: 'Student Hub', app_sub: 'TU-Varna',
  nav_dashboard: 'Home', nav_schedule: 'Timetable', nav_tasks: 'Tasks', nav_notes: 'Notes', nav_grades: 'Grades',
  nav_docs: 'Documents', nav_focus: 'Focus', nav_links: 'Links', nav_settings: 'Settings', nav_profile: 'Profile', nav_more: 'More',
  close: 'Close', cancel: 'Cancel', delete: 'Delete', save: 'Save', add: 'Add', edit: 'Edit', are_you_sure: 'Are you sure?',
  d_short: 'd', today: 'Today', tomorrow: 'Tomorrow', yesterday: 'Yesterday', in_days: 'in {0} days', days_ago: '{0} days ago',
  theme: 'Theme', theme_auto: 'Auto', theme_light: 'Light', theme_dark: 'Dark', language: 'Language',
  sign_out: 'Sign out', loading: 'Loading…', retry: 'Try again', offline: 'Offline — showing saved data.',

  hero_title_1: 'Your whole degree,', hero_title_2: 'in one place.',
  hero_lead: 'Your exact timetable from the TU-Varna website, tasks and deadlines, notes, grades, and an assistant that drafts documents for you — sign in with your university Microsoft account.',
  sign_in_ms: 'Sign in with Microsoft', try_demo: 'Try the demo', ms_not_configured: 'Microsoft sign-in is not set up on this server yet.',
  trust_local: 'Your data stays in your browser', trust_no_pw: 'No passwords stored', trust_captcha: 'Bot protection',
  f_schedule: 'Accurate timetable', f_schedule_d: 'Pick specialty, year and group — we read it straight from umo.tu-varna.bg, with sub-groups and odd/even weeks.',
  f_docs: 'Document assistant', f_docs_d: 'Reports, coursework, lab reports, e-mails to lecturers, CVs — a draft in seconds, export to Word.',
  f_tasks: 'Tasks & exams', f_tasks_d: 'Deadlines, homework and the exam session with countdowns.',
  f_grades: 'Grades', f_grades_d: 'Credit-weighted average on the Bulgarian 2–6 scale, per semester.',
  f_focus: 'Focus timer', f_focus_d: 'Pomodoro sessions and how much you studied this week.',
  f_private: 'Private', f_private_d: 'Everything personal stays in your browser. The server keeps only a temporary session, no passwords or tokens on disk.',
  auth_error_title: 'Sign-in failed', perm_error_title: 'Microsoft access was not granted',
  ae_captcha: 'Please confirm you are human first.', ae_session: 'The session expired. Please try again.', ae_expired: 'The sign-in attempt expired. Please try again.',
  ae_cancelled: 'Sign-in was cancelled.', ae_consent: 'The university must approve this app (admin consent). Please contact the administrator.',
  ae_domain: 'Please sign in with your university account (@tu-varna.bg).', ae_not_configured: 'Microsoft sign-in is not configured.', ae_rate_limited: 'Too many attempts. Wait a minute.',
  ae_generic: 'Something went wrong during sign-in. Please try again.',

  cap_title: 'Confirm you are human', cap_hint: 'Drag the blue dot onto the empty circle.',
  cap_working: 'Checking your browser…', cap_ready: 'Ready — drag the dot.', cap_checking: 'Checking…', cap_ok: 'Confirmed',
  cap_bad: 'That did not work. Here is a new puzzle.', cap_cross_site: 'The server does not accept requests from this address. Open the site at the address the server shows when it starts.', cap_keys: 'Keyboard: ← → and Enter.', cap_new: 'New puzzle',

  good_morning: 'Good morning', good_day: 'Good afternoon', good_evening: 'Good evening',
  week_n: 'Week {0}', odd_week: 'odd', even_week: 'even', before_semester: 'before the semester',
  today_classes: 'Today', no_classes_today: 'No classes today.', set_up_schedule: 'Set up your timetable', set_up: 'Set up',
  next_class: 'Next class', starts_in: 'starts in', now_in: 'Now', ends_in: 'ends in', no_more_today: 'No more classes today.',
  due_soon: 'Due soon', nothing_due: 'Nothing due soon.', overdue_n: '{0} overdue', exams: 'Exams', no_exams: 'No exams added.',
  avg_grade: 'Average grade', no_grades: 'No grades yet.', focus_today: 'Focus today', minutes_short: 'min',
  quick_add: 'Quick task…', open: 'Open', more_n: '+ {0} more →', no_links: 'No links of your own yet. Add the first one above.', mark_done: 'Mark as done',

  sch_setup: 'Your group', semester: 'Semester', winter: 'Winter', summer: 'Summer', specialty: 'Specialty', course: 'Year', group: 'Group',
  load_schedule: 'Load timetable', or_paste_url: 'or paste a timetable address from umo.tu-varna.bg', url: 'Address', load: 'Load',
  subgroup: 'Sub-group', all: 'All', show: 'Show', this_week_only: 'This week only', every_week: 'Every week',
  updated: 'Updated {0}', refresh: 'Refresh', source: 'Source', change_group: 'Change group',
  k_lecture: 'Lecture', k_lab: 'Lab', k_seminar: 'Seminar', k_exercise: 'Exercise', k_seminar_lab: 'Seminar + lab', k_project: 'Course project', k_other: 'Other',
  odd_only: 'odd weeks', even_only: 'even weeks', days: ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'],
  days_short: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'],
  sem_start_hint: 'Week numbers count from the semester start ({0}). If it does not match, change it in Settings.',
  sch_notes: 'Notes on the timetable', no_lessons: 'This timetable has no classes.', week_prev: 'Previous week', week_next: 'Next week', this_week: 'This week',
  ms_calendar: 'Microsoft calendar', ms_calendar_none: 'No calendar access (the university does not use Outlook for mail).',
  err_host_not_allowed: 'Only tu-varna.bg addresses are allowed.', err_no_timetable_for_group: 'No timetable for this combination.',
  err_upstream: 'The university website is not answering. Try again shortly.', err_parse: 'That file does not look like a TU-Varna timetable.',

  new_task: 'New task', title: 'Title', subject: 'Subject', due: 'Due', type: 'Type', priority: 'Priority',
  t_task: 'Task', t_homework: 'Homework', t_exam: 'Exam', t_project: 'Project', p_low: 'Low', p_normal: 'Normal', p_high: 'High',
  g_overdue: 'Overdue', g_today: 'Today', g_week: 'This week', g_later: 'Later', g_nodate: 'No date', g_done: 'Done',
  no_tasks: 'No tasks yet. Add the first one!', filter_all: 'All', filter_open: 'Open', filter_exams: 'Exams', clear_done: 'Clear done',

  new_note: 'New note', search: 'Search…', untitled: 'Untitled', saved_locally: 'Saved in this browser', no_notes: 'No notes yet.',
  preview: 'Preview', write: 'Write', pick_note: 'Pick or create a note.',

  add_subject: 'Add subject', credits: 'Credits', grade: 'Grade', weighted_avg: 'Average (credit-weighted)', simple_avg: 'Simple average',
  total_credits: 'Credits', gr_6: 'Excellent', gr_5: 'Very good', gr_4: 'Good', gr_3: 'Satisfactory', gr_2: 'Fail', not_graded: '—',
  grades_hint: 'Bulgarian 2–6 scale. The average is weighted by credits (ECTS).',

  doc_assistant: 'Document assistant', what: 'What', action: 'Action', topic: 'Topic', details: 'Requirements and notes', details_ph: 'E.g.: 10 pages, aim, what to cover, sources you have…',
  length: 'Length', short: 'Short', medium: 'Medium', long: 'Long', tone: 'Tone', academic: 'Academic', neutral: 'Neutral', friendly: 'Friendly',
  include_profile: 'Add my name and specialty (for a title page)', generate: 'Create', stop: 'Stop',
  a_write: 'Write', a_outline: 'Outline', a_improve: 'Improve', a_continue: 'Continue', a_summarize: 'Summary', a_questions: 'Questions', a_translate: 'Translate',
  d_essay: 'Essay', d_report: 'Report', d_coursework: 'Coursework', d_lab: 'Lab report', d_summary: 'Study summary', d_email: 'E-mail to lecturer',
  d_cv: 'CV', d_presentation: 'Presentation', d_letter: 'Application letter', d_notes: 'Study notes', d_other: 'Other',
  my_docs: 'My documents', new_doc: 'New document', export_docx: 'Word', export_md: 'Markdown', print: 'Print / PDF', no_docs: 'No documents yet.',
  ai_on: 'AI: {0}', ai_off: 'Offline templates (AI is not enabled on the server)', thinking: 'Thinking…', writing: 'Writing…', template_mode: 'Template',
  ai_errors_hourly_limit: 'Hourly limit reached. Try later.', ai_errors_daily_limit: 'The site’s daily limit is reached.',
  ai_errors_ai_busy: 'The AI service is busy. Try again shortly.', ai_errors_ai_key: 'The server’s AI key is not valid.', ai_errors_refusal: 'The assistant declined this request.',
  ai_errors_generic: 'Generation failed. Try again.', ai_errors_no_topic: 'Write a topic.', ai_errors_no_text: 'The document needs some text first.',
  ai_note: 'Check the facts. The assistant leaves [placeholders] for your data and sources.', words: '{0} words',
  apply_to_doc: 'The actions below work on the text in the editor.',

  focus: 'Focus', break: 'Break', start: 'Start', pause: 'Pause', reset: 'Reset', skip: 'Skip', this_week_focus: 'This week', sessions_today: 'Sessions today',
  focus_done: 'Session done — take a break!', break_done: 'Break over — back to it!',

  uni_links: 'TU-Varna', my_links: 'My links', add_link: 'Add link', name: 'Name',
  l_site: 'Official website', l_umo: 'Timetables (UMO)', l_elearning: 'eTU-Varna (Moodle)', l_sis: 'Student information system',
  l_mail: 'University mail', l_teams: 'Microsoft Teams', l_office: 'Microsoft 365',

  appearance: 'Appearance', semester_start: 'Semester start (week 1)', semester_start_hint: 'The Monday of the first teaching week. Odd/even weeks count from it.',
  my_subgroup: 'My sub-group', data: 'Data', export_data: 'Download a copy (JSON)', import_data: 'Restore from a copy', clear_data: 'Delete everything on this device',
  data_hint: 'Tasks, notes, grades and documents are kept only in this browser. Download a copy if you switch devices.',
  privacy: 'Privacy', privacy_text: 'The server writes nothing to disk. It keeps a temporary session in memory (a random id in an HttpOnly cookie) and the Microsoft token for up to an hour. Your password never passes through us — you type it only on Microsoft’s page.',
  imported: 'Data restored.', import_failed: 'That file is not a valid copy.', cleared: 'Deleted.',

  from_microsoft: 'From your Microsoft account', email: 'E-mail', department: 'Department / specialty', job_title: 'Job title', office: 'Office',
  faculty_number: 'Number (faculty no.)', organization: 'Organization', permissions: 'Permissions', demo_account: 'Demo account',
};

const DICTS = { bg: BG, en: EN };
let current = document.documentElement.lang === 'en' ? 'en' : 'bg';

export function lang() { return current; }
export function setLang(l) { current = l === 'en' ? 'en' : 'bg'; document.documentElement.lang = current; }

export function t(key, ...args) {
  const v = DICTS[current][key] ?? BG[key] ?? key;
  if (typeof v !== 'string') return v;
  return args.length ? v.replace(/\{(\d+)\}/g, (_, i) => String(args[+i] ?? '')) : v;
}
