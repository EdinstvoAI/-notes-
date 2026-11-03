# Карта проекта: что где лежит

Каждый файл кода — одной строкой. `DocsTests` падает, если файл есть в коде, но его нет здесь, или
наоборот: новый файл — сразу строкой сюда, удалённый — убрать. Подробности — в заглавном комментарии
самого файла.

## Приложение (`<notes>/`)

- `<notes>/MyApp.swift` — точка входа (@main): окно, запуск синхронизации, входа и надиктовки, меню «Отладка».
- `<notes>/ThemeCrossfade.swift` — плавная смена светлой и тёмной темы: снимок старой темы растворяется.
- `<notes>/Theme.swift` — цвета и шрифты из макета, регистрация шрифтов Martian Grotesk.
- `<notes>/DotIcon.swift` — точечные иконки (битмапы как в Figma).
- `<notes>/GitHubApp.swift` — client id приложения на GitHub для входа по коду.
- `<notes>/Onboarding/OnboardingView.swift` — сцена: загрузка, планки, лист, логотип, финал, вход, окно пинкода поверх.
- `<notes>/Onboarding/OnboardingModel.swift` — состояние первого входа и входа: фаза, шаг, блокировка, сброс пинкода.
- `<notes>/Onboarding/OnboardingSteps.swift` — содержимое шагов 1–9 и общий каркас шага.
- `<notes>/Onboarding/OnboardingRows.swift` — строки списков шагов 6–9: модели, скачивание, разрешения, пароль.
- `<notes>/Onboarding/GlassSlats.swift` — стеклянные планки шапки, нарисованные вручную.
- `<notes>/Onboarding/GitHubLogin.swift` — вход в GitHub по коду: опрос, создание `notes`, первая отправка.
- `<notes>/Onboarding/HotKeyRow.swift` — строка «горячая клавиша»: запись нового сочетания.
- `<notes>/Onboarding/Permissions.swift` — статусы и запросы доступа к микрофону и календарю.
- `<notes>/Onboarding/PinEntrySheet.swift` — окно пинкода поверх шага, ячейки, клавиши.
- `<notes>/Onboarding/UnlockStep.swift` — вход: пинкод, Touch ID, пауза после ошибок, «забыл пинкод».
- `<notes>/MCP/MCPScreen.swift` — раздел MCP: подключение Claude Code / Codex / Cursor, промты и скилл, закрытые папки, только чтение, журнал обращений.
- `<notes>/Vault/SyncScreen.swift` — раздел «синхронизация»: версии из git по дням, файлы версии, отмена правок и откат ко всей версии.
- `<notes>/Notes/NotesModel.swift` — состояние главного экрана: выбор, автосохранение, правки снаружи, поиск, самопроверка.
- `<notes>/Notes/FilePreview.swift` — просмотр файлов хранилища (PDF, Word, фото, видео) встроенным Quick Look и «открыть в программе».
- `<notes>/Notes/NoteTabs.swift` — вкладки открытых заметок над листом: открыть, закрыть, переход на соседнюю.
- `<notes>/Notes/NoteActions.swift` — меню заметки и папки: закрепить, копировать, вставить, дублировать, удалить в Корзину.
- `<notes>/Notes/NotesWindow.swift` — главный экран: шапка, боковая панель, лист содержимого, кнопки.
- `<notes>/Notes/NoteViews.swift` — заметка, обзор папок с карточками, строки, поиск поверх.
- `<notes>/Notes/MarkdownEditor.swift` — редактор текста заметки (NSTextView) с оформлением заголовков.
- `<notes>/Tasks/TasksModel.swift` — задачи из заметок, фильтры, отметка; синхронизация с Apple Calendar (EventKit).
- `<notes>/Tasks/TasksScreen.swift` — экраны задач и календаря (неделя, день, месяц).
- `<notes>/Video/VideoModel.swift` — очередь расшифровок, хранение вне хранилища, заметка, копия, .srt.
- `<notes>/Icon/DockIcon.swift` — иконка в Доке темнеет вместе с темой приложения; `icon-dark.png` рядом собрана из слоёв AppIcon.icon.
- `<notes>/Video/DropZone.metal` — шейдер зоны «перетащи» в видео: стеклянные планки поверх плывущих тёплых пятен.
- `<notes>/Video/VideoScreen.swift` — список видео с зоной для файла и экран расшифровки.
- `<notes>/Settings/SettingsScreen.swift` — страницы настроек на настоящих данных: общие, синхронизация, надиктовка, поиск, вход, календарь, шаблоны.
- `<notes>/Localizable.xcstrings` — переводы интерфейса: en и sr-Latn, исходный русский.
- `<notes>/AppSupport.swift` — своя папка в Application Support и переезд данных из контейнера песочницы.
- `<notes>/Dictation/AutoPaste.swift` — вставка надиктованного в поле в фокусе (универсальный доступ, ⌘V).
- `<notes>/Intelligence/Intelligence.swift` — Apple Intelligence (Foundation Models): включение, доступность, краткое содержание по частям.
- `<notes>/Settings/AppSettings.swift` — подробные настройки в UserDefaults: тема, запуск при входе, размер текста, орфография, расписание синхронизации, язык речи, длина записи.
- `<notes>/Vault/VaultStore.swift` — папка хранилища: выбор, security-scoped bookmark, iCloud.
- `<notes>/Vault/BiometricLock.swift` — Touch ID: включить, войти, подтвердить хозяина для сброса пинкода.
- `<notes>/Vault/SyncController.swift` — раз в час: коммит, pull, отправка раз в сутки.
- `<notes>/Vault/AppLock.swift` — умный вход: блокировка и сон Mac, 15 минут в фоне, отметка для перезапуска.
- `<notes>/Dictation/DictationController.swift` — надиктовка: хоткей, запись, распознавание, буфер, сфера.
- `<notes>/Dictation/GlobalHotKey.swift` — глобальный хоткей через Carbon, без «Универсального доступа».
- `<notes>/Dictation/AudioRecorder.swift` — запись микрофона в 16 кГц моно.
- `<notes>/Dictation/DictationOrb.swift` — сфера с волнами от громкости и галочкой под стеклом; `OrbLayer`/`OrbRenderer` — свой Metal-слой (MTKView) для шейдера; `OrbClock` — сглаживание голоса и замедление после речи.
- `<notes>/Dictation/Orb.metal` — шейдер сферы: стеклянный шар преломляет полосу света
- `<notes>/Dictation/SpeechSettings.swift` — какая модель выбрана, подписи моделей.
- `<notes>/Dictation/ModelDownloads.swift` — одно скачивание модели на всё приложение и окно «скачать модель?».

## Логика без интерфейса (`NotesCore/`)

- `NotesCore/Package.swift` — пакет и зависимость libgit2.
- `NotesCore/Sources/NotesCore/GitVault.swift` — git в папке: init, `.gitignore`, коммиты, remote, push.
- `NotesCore/Sources/NotesCore/GitVault+Pull.swift` — clone, fetch, pull, конфликты без потери текста.
- `NotesCore/Sources/NotesCore/GitHub.swift` — вход по коду (Device Flow), создание приватного `notes`.
- `NotesCore/Sources/NotesCore/TokenStore.swift` — записи в Связке ключей, пересоздание старых без запросов.
- `NotesCore/Sources/NotesCore/Sync.swift` — подпись коммита, расписание отправки, движок синхронизации.
- `NotesCore/Sources/NotesCore/NoteLibrary.swift` — заметки в папке: дерево, чтение и запись, создание, переименование, поиск по тексту.
- `NotesCore/Sources/NotesCore/NoteSections.swift` — разделы от Apple Intelligence в заметке: «## Кратко» и «## Задачи».
- `NotesCore/Sources/NotesCore/NoteTasks.swift` — задачи из строк заметок: разбор, отметка, группы, план синхронизации с календарём.
- `NotesCore/Sources/NotesCore/Transcript.swift` — расшифровка: куски с временем, .srt, заметка Markdown.
- `.github/workflows/release.yml` — релиз по тегу: DMG на macOS и релиз с текстом из CHANGELOG.
- `NotesCore/Sources/NotesCore/GitVault+History.swift` — история коммитов с файлами, отмена одного коммита и откат к версии новым коммитом.
- `NotesCore/Tests/NotesCoreTests/GitHistoryTests.swift` — тесты истории: порядок, отмена, откат, конфликт.
- `NotesCore/Sources/NotesCore/MCPServer.swift` — локальный MCP-сервер для нейронок: поиск, чтение, новые заметки и задачи, только внутри хранилища.
- `NotesCore/Sources/notes-mcp/main.swift` — исполняемый `notes-mcp --vault <папка>`: stdio-обёртка над MCPServer.
- `NotesCore/Tests/NotesCoreTests/MCPServerTests.swift` — тесты MCP: рукопожатие, инструменты, выход за хранилище.
- `NotesCore/Sources/NotesCore/Templates.swift` — шаблоны из папки «шаблоны» с подстановками {{дата}}, {{время}}, {{название}} и заметка на каждый день в «дневник».
- `NotesCore/Sources/NotesCore/SearchQuery.swift` — запрос общего поиска: все слова без регистра и ё/е, упоминание в расшифровке.
- `NotesCore/Sources/NotesCore/SemanticIndex.swift` — поиск по смыслу: куски заметок, векторы встроенной модели Apple. С 31.10.2026 в приложении не подключён, ждёт модели получше.
- `NotesCore/Sources/NotesCore/FolderWatcher.swift` — слежка за папкой (FSEvents), `.git` пропускаем.
- `NotesCore/Sources/NotesCore/HotKeyCombo.swift` — сочетание надиктовки: подпись, проверка, хранение.
- `NotesCore/Sources/NotesCore/PinCode.swift` — пинкод: ввод, проверка при входе, хеш, хранение.
- `NotesCore/Sources/NotesCore/LockPolicy.swift` — когда спрашивать пинкод, пауза после ошибок.
- `NotesCore/Tests/NotesCoreTests/*.swift` — тесты: git, GitHub, синхронизация, pull, хоткей, пинкод, вход, документация.

## Распознавание речи (`NotesSpeech/`)

- `NotesSpeech/Package.swift` — пакет: sherpa-onnx (готовая сборка) и WhisperKit.
- `NotesSpeech/Sources/NotesSpeech/SpeechModel.swift` — каталог моделей, скачивание, проверка суммы.
- `NotesSpeech/Sources/NotesSpeech/MediaTranscription.swift` — звук из видео и аудио → 16 кГц, куски по 20 с или по репликам спикеров, распознавание.
- `NotesSpeech/Sources/NotesSpeech/Diarization.swift` — спикеры: скачивание моделей голосов и разметка «кто когда говорит».
- `NotesSpeech/Sources/NotesSpeech/Transcriber.swift` — распознавание: GigaAM через sherpa-onnx, Whisper.
- `NotesSpeech/Tests/NotesSpeechTests/*.swift` — тесты: с настоящими моделями (включаются переменными) и нарезка файла с заглушкой.
- `NotesSpeech/Tests/NotesSpeechTests/SpeechTests.swift` — тесты с настоящими моделями (включаются переменными).

## Прочее

- `Scripts/fetch-sherpa-onnx.sh` — скачивает sherpa-onnx, проверяет суммы, собирает xcframework.
- `Scripts/make-dmg.sh` — собирает установщик build/notes.dmg: Release, окно с фоном, ярлык «Программы».
- `Scripts/dmg-background.swift` — рисует фон установщика: полоски как в шапке, коралловая стрелка.
- `<notes>/AppIcon.icon` — иконка приложения (Icon Composer): слой планок и слой надписи, светлая и тёмная.
- `notes.entitlements` — права, которых нет в настройках сборки (bookmarks).
- `<notes>.xcodeproj` — проект Xcode, пакеты подключены локально.
