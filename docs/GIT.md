# Ветки, коммиты, релизы

## Коммиты

```
<Feature|Fix> <Модуль> / <Описание с заглавной буквы>
```

Модули: Onboarding, Notes, Editor, Sync, Search, Dictation, Tasks, Calendar, Settings, Dev, Docs, Windows.
Описание — по-английски, в повелительном наклонении, без точки в конце.

Тело — по-русски и по делу: что было сломано, почему сломалось и почему решено именно так.
Список файлов не нужен, он виден в диффе. В конце — строка `Co-Authored-By`.

Коммитить можно без вопроса, когда кусок работы доделан и проверен (сборка, тесты, вживую) —
так решил Макар для этого проекта ([DECISIONS.md](DECISIONS.md)). В «Единстве» иначе: там код — по его слову.
Правки документации коммитятся сразу, отдельным коммитом `Feature Docs / …`, без кода в нём,
когда `DocsTests` зелёный: договорённости не теряются, даже если сессия оборвётся.
В коммит идёт только своё и законченное: `git add` по файлам, если в дереве лежит чужое.
Пушить — только когда попросили. Вся работа идёт в ветке `Dev`, `main` — только для релизов.

## Релизы

Первый — `v1.0.0`, 01.11.2026 («сделаем релиз 1 версия, при этом на англе лог»). Версии — semver.
Порядок, только по слову Макара:

1. `CHANGELOG.md` в корне — новый раздел сверху, **по-английски**, по разделам приложения.
2. `MARKETING_VERSION` в проекте — тот же номер.
3. `Scripts/make-dmg.sh`, Dev — fast-forward в `main`, тег `vX.Y.Z` на `main`, пуш `main`, `Dev` и тега.
   Пуш тега запускает `.github/workflows/release.yml`: сборка DMG на macOS (подпись ad-hoc) и релиз
   с разделом из CHANGELOG. Если релиз уже создан руками — action его не трогает (03.11.2026).
4. Руками, если action не справился: `gh release create vX.Y.Z build/notes.dmg --title "<notes> X.Y.Z" --notes-file …` — текст из раздела
   CHANGELOG.

**Бета — не в main** (03.11.2026, «это же бетка»): тег беты ставится на `Dev-macOS`, релиз — pre-release,
`main` не трогаем. В `main` — только стабильные версии.

Название релиза — **только имя и версия**: `<notes> 1.0.0`, без описания в названии. Описание —
по-английски: раздел CHANGELOG и в конце блок «Install» про Gatekeeper (01.11.2026, [DECISIONS.md](DECISIONS.md)).

Облачная сессия релиз не создаст: GitHub отвечает 403 «not permitted for this session type»
(01.11.2026), DMG на Linux тоже не собрать. Релиз делает Макар на маке, шаги ниже. Если релиз уже есть,
DMG докладывается так:

```
gh release upload v1.0.0 build/notes.dmg --clobber
```

### DMG руками, по кликам

Красивый установщик (фон, приложение слева, ярлык «Программы» справа) собирает только скрипт: в Xcode
этого нет. Xcode нужен, только чтобы проверить подпись.

1. Xcode → открыть `<notes>.xcodeproj` → в левой колонке синяя иконка `<notes>` → таргет `<notes>` →
   вкладка **Signing & Capabilities** → **Team**: Personal Team (U8T962NCZ2). Закрыть Xcode не обязательно.
2. Терминал → `cd` в папку репозитория → `git checkout main` (релиз собирается с `main`, не с `Dev`).
3. `Scripts/make-dmg.sh` — в конце строка `13M build/notes.dmg`. Во время сборки на секунду откроется
   окно Finder с установщиком: это скрипт расставляет иконки, его не трогать.
4. Проверить: `hdiutil verify build/notes.dmg`, двойной клик по файлу — окно с фоном и стрелкой.
5. Текст релиза — раздел CHANGELOG плюс блок «Install»:

   ```
   awk '/^## 1.0.0/{f=1;next} /^## /{if(f)exit} f' CHANGELOG.md > build/release.md
   cat >> build/release.md <<'EOF'

   ### Install
   1. Download `notes.dmg` below, open it and drag `<notes>` into Applications.
   2. This build is signed for development and not notarized yet. On first launch macOS will block it:
      open System Settings → Privacy & Security, scroll down and click **Open Anyway**.

   Requires macOS 27 or later.
   EOF
   ```

6. `gh release create v1.0.0 build/notes.dmg --title "<notes> 1.0.0" --notes-file build/release.md --latest`
   — релиз с DMG. В конце ссылка на страницу релиза.
7. `git checkout Dev` — вернуться в рабочую ветку.

`build/notes.dmg` в git не идёт (`build/` в `.gitignore`).

Сборка подписана сертификатом Apple Development и не нотаризована: на чужом Mac Gatekeeper её не
пускает, пока не нажать «Всё равно открыть» в настройках «Конфиденциальность и безопасность».
Подпись Developer ID и нотаризация — когда Макар даст аккаунт разработчика.
