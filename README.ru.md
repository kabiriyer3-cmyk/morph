<p align='center'>
  <img src='assets/banner.svg' alt='Morph' width='100%'>
</p>

<p align='center'>
  <a href='https://github.com/kabiriyer3-cmyk/morph/releases/latest'><img alt='Release' src='https://img.shields.io/github/v/release/kabiriyer3-cmyk/morph?style=flat-square&color=2f6ff2'></a>
  <a href='https://github.com/kabiriyer3-cmyk/morph/actions/workflows/ci.yml'><img alt='Build' src='https://img.shields.io/github/actions/workflow/status/kabiriyer3-cmyk/morph/ci.yml?branch=main&style=flat-square&label=build'></a>
  <a href='https://github.com/kabiriyer3-cmyk/morph/releases'><img alt='Downloads' src='https://img.shields.io/github/downloads/kabiriyer3-cmyk/morph/total?style=flat-square&color=1259e0'></a>
  <img alt='Platforms' src='https://img.shields.io/badge/platforms-Windows%20%7C%20Linux-0b1530?style=flat-square'>
  <a href='LICENSE'><img alt='License' src='https://img.shields.io/badge/license-MIT-4a92ff?style=flat-square'></a>
</p>

<p align='center'><a href='README.md'>English</a> · <b>Русский</b></p>

**Morph** — бесплатное приложение с открытым кодом для сжатия, конвертации и пакетной обработки файлов через нодовый граф, как в ComfyUI, только для обычных файлов: фото, видео, аудио, 3D-моделей, архивов, данных и документов. Соедините несколько нод, перетащите папку на холст и нажмите **Запуск**. Оригиналы никогда не изменяются.

<p align='center'><img src='docs/screenshot.png' alt='Скриншот Morph' width='100%'></p>

## Почему Morph

- **Нодовые графы.** Собирайте шаги наглядно: уменьшить, конвертировать, поставить водяной знак, сохранить. Ветвления, объединения, пропуск нод. Графы сохраняются в `.morph.json`, ими можно делиться.
- **Один инструмент для всех форматов.** Изображения, видео, аудио, 3D, архивы, данные, офисные документы и разметка в одном окне.
- **Гибкие настройки под каждый формат.** Качество и целевой размер WebP/AVIF, палитра и оптимизация PNG, CRF и пресеты, кодирование на видеокарте, нормализация громкости, упрощение 3D-сеток и многое другое.
- **Превью везде.** Миниатюры прямо в графе, слайдер «до/после» для картинок, плеер видео и аудио, интерактивный 3D-просмотрщик моделей.
- **Быстро и приватно.** Бэкенд на Rust, пакетная обработка на всех ядрах, файлы не покидают компьютер. Без аккаунтов и телеметрии.
- **Безопасно.** Каждая нода пишет во временную рабочую папку. На диск пишет только нода «Сохранить», и она ничего не перезаписывает без вашего разрешения.

## Скачать

| Платформа | Пакет | |
|---|---|---|
| Windows 10/11 x64 | Установщик `Morph_x.y.z_x64-setup.exe` | [Последний релиз](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Windows 10/11 x64 | MSI `Morph_x.y.z_x64_en-US.msi` | [Последний релиз](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Windows 10/11 x64 | Портативная версия `Morph_x.y.z_x64_portable.exe`, без установки | [Последний релиз](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Linux x64 | AppImage, работает на большинстве дистрибутивов | [Последний релиз](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Debian / Ubuntu / Mint | `.deb` | [Последний релиз](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |
| Fedora / openSUSE | `.rpm` | [Последний релиз](https://github.com/kabiriyer3-cmyk/morph/releases/latest) |

`.deb` ставится командой `sudo apt install ./Morph_*_amd64.deb`, `.rpm` — `sudo dnf install ./Morph-*.x86_64.rpm`, а AppImage нужно сделать исполняемым: `chmod +x Morph_*.AppImage`.

## Быстрый старт

1. Откройте Morph. Сразу загружен шаблон **Фото для сайта**.
2. Перетащите на холст папку или фотографии — они попадут в ноду **Загрузить файлы**.
3. Нажмите **Запуск** или `Ctrl+Enter`. Миниатюры появятся в **Предпросмотре**, а результаты — в папке `~/Morph Output` (меняется в ноде **Сохранить**).

Другие шаблоны — на боковой панели, а двойной клик по холсту открывает поиск любой ноды. Язык интерфейса (русский или английский) выбирается автоматически и меняется в настройках.

## Ноды

| Категория | Ноды |
|---|---|
| **Файлы** | Загрузить, Сохранить, Предпросмотр, Фильтр (тип, маска, регулярка, размер), Разделить по типу, Объединить, Переименовать (шаблоны `{name}` `{n:3}` `{date}`, поиск и замена, регистр), Контрольные суммы (SHA-256, SHA-1, SHA-512, MD5, BLAKE3, CRC32), Убрать дубликаты, Сведения о файлах (отчёт CSV или JSON) |
| **Изображения** | Изменить размер, Обрезать (пропорции, область, автообрезка полей), Повернуть / отразить, Коррекция (яркость, контраст, насыщенность, оттенок, гамма, размытие, резкость), Конвертация (WebP, AVIF, JPEG, PNG, GIF, BMP, TIFF, ICO, TGA, QOI: качество, без потерь, целевой размер в КБ, оптимизация и палитра PNG), Водяной знак, Контактный лист, Иконки (.ico и PNG), Изображения в PDF |
| **Видео** | Конвертация (MP4, MKV, WebM, MOV: H.264, H.265, VP9, AV1, ProRes, CRF, пресеты, целевой размер, разрешение, fps, NVENC, QSV, AMF), Обрезка, Трансформация (поворот, отражение, кадрирование, скорость, без звука), Видео в GIF или анимированный WebP, Извлечь кадры, Извлечь аудио, Склеить видео |
| **Аудио** | Конвертация (MP3, AAC, Opus, OGG, FLAC, WAV, ALAC: битрейт, частота, каналы), Обработка (громкость, нормализация, обрезка, затухания, скорость) |
| **3D** | Конвертация (GLB / glTF, OBJ, STL, PLY, OFF), Трансформация (масштаб, единицы, поворот, сдвиг, центрирование, постановка на пол, вписывание в размер, ось «вверх», разворот нормалей), Оптимизация (сварка вершин, упрощение, сглаженные или плоские нормали) |
| **Архивы** | Создать (ZIP, 7z, tar, tar.gz, tar.zst, tar.xz, tar.bz2: один архив или по архиву на файл), Распаковать, Сжать отдельные файлы (zstd, gzip, xz, bzip2) |
| **Данные и документы** | Конвертация данных (JSON, YAML, TOML, CSV, TSV, NDJSON), Обработка текста (кодировки, включая Windows-1251 и KOI8-R, переводы строк, пробелы, поиск и замена), Office-конвертация (DOCX, XLSX, PPTX, ODT, PDF и другие через LibreOffice), Pandoc-конвертация (Markdown, HTML, DOCX, EPUB, LaTeX, RST) |
| **Прочее** | Заметка |

Любая нода принимает любые файлы: те, что ей не по профилю, проходят дальше без изменений. Поэтому в цепочку можно подать целую папку вперемешку.

## Шаблоны

Фото для сайта · Сжать видео · Видео в GIF · Конвертировать всё · 3D для веба · Бэкап с контрольными суммами · Фото в PDF · Иконки приложения · JSON в YAML · Найти дубликаты

## Горячие клавиши

| Клавиши | Действие |
|---|---|
| `Ctrl+Enter` | Запуск или остановка |
| `Tab` или двойной клик по холсту | Найти и добавить ноду |
| `Delete` | Удалить выбранные ноды |
| `Ctrl+C` / `Ctrl+V` / `Ctrl+D` | Копировать / вставить / дублировать |
| `Ctrl+Z` / `Ctrl+Y` | Отменить / повторить |
| `Ctrl+B` | Пропускать выбранные ноды |
| `F` | Показать весь граф |
| `Пробел` + перетаскивание или средняя кнопка мыши | Двигать холст |
| Колесо мыши | Масштаб |
| `Ctrl` + перетаскивание | Выделение рамкой |
| `Ctrl+S` / `Ctrl+O` | Сохранить / открыть граф |

## Дополнительные программы

Изображения, 3D, архивы, данные и текст работают сразу. Несколько групп нод используют известные бесплатные программы. Morph находит их сам (PATH, стандартные папки установки или папка `tools` рядом с `Morph.exe`), а пути можно задать и вручную в **Настройках**.

| Программа | Для чего | Windows | Linux |
|---|---|---|---|
| [FFmpeg](https://ffmpeg.org) | ноды видео и аудио, миниатюры видео и аудио | `winget install Gyan.FFmpeg` | `sudo apt install ffmpeg` |
| [LibreOffice](https://ru.libreoffice.org) | Office-конвертация | `winget install TheDocumentFoundation.LibreOffice` | `sudo apt install libreoffice` |
| [Pandoc](https://pandoc.org) | Pandoc-конвертация | `winget install JohnMacFarlane.Pandoc` | `sudo apt install pandoc` |

В Fedora — `sudo dnf install ...` (FFmpeg берётся из [RPM Fusion](https://rpmfusion.org/Configuration)), в Arch — `sudo pacman -S ffmpeg libreoffice-fresh pandoc`.

## Сборка из исходников

Нужны [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 20+ и, на Linux, пакеты разработки WebKitGTK:

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libgtk-3-dev build-essential
```

На Windows нужны Visual Studio C++ Build Tools и WebView2 (уже есть в Windows 10 и 11).

```bash
git clone https://github.com/kabiriyer3-cmyk/morph.git
cd morph
npm install
npm run dev     # запустить приложение
npm run build   # установщики появятся в src-tauri/target/release/bundle
```

Интерфейс — обычные HTML, CSS и JavaScript в `src/`, без сборщика. Его можно попробовать в браузере на встроенных демо-данных:

```bash
python3 -m http.server 8080 --directory src
# затем откройте http://localhost:8080/?demo=1
```

`npm run check` запускает статические проверки фронтенда, а `npm run smoke` рендерит демо в headless Chrome.

## Структура проекта

```
src/                 интерфейс: index.html, css/, js/ (редактор, виджеты, инспектор, 3D, i18n)
src-tauri/           бэкенд на Rust (Tauri 2)
  src/engine.rs      исполнение графа, события прогресса, рабочие папки
  src/ops/           реализации нод: files, img, media (FFmpeg), mesh3d, arch, data, docs
  src/mesh.rs        чтение и запись 3D, сварка и упрощение сеток
assets/              логотип, иконка и баннер (иконки приложения генерируются из assets/icon.svg при сборке)
.github/actions/     шаги CI и релиза
scripts/             проверки фронтенда, smoke-тест, аннотации ошибок cargo
```

## Выпуск релиза

1. Поднимите `version` в `src-tauri/tauri.conf.json` и `src-tauri/Cargo.toml`.
2. Смерджите в `main`. Workflow **Release** соберёт пакеты для Windows и Linux на GitHub Actions и опубликует релиз `vX.Y.Z` с установщиком, MSI, портативным exe, AppImage, deb и rpm.

Также можно запушить тег `vX.Y.Z` или запустить **Release** вручную на вкладке Actions. Текст релиза берётся из `.github/release-notes.md`.

## Частые вопросы

**Windows SmartScreen пишет, что издатель неизвестен.** Сборки пока не подписаны. Нажмите *Подробнее*, затем *Выполнить в любом случае*, или возьмите портативную версию.

**AppImage не запускается.** Сделайте файл исполняемым: `chmod +x`. На некоторых дистрибутивах нужен ещё `libfuse2` (`sudo apt install libfuse2`).

**Ноды видео пишут, что FFmpeg не найден.** Установите FFmpeg (см. [Дополнительные программы](#дополнительные-программы)) и нажмите *Найти снова* в настройках или укажите путь к ffmpeg вручную.

**Куда пишутся промежуточные файлы?** В `morph/runs` во временной папке системы. Хранятся только последние запуски, а *Настройки, Очистить кэш* удаляет их.

**Morph куда-нибудь отправляет мои файлы?** Нет. Всё работает локально, ничего не передаётся по сети.

**Справится с тысячами файлов?** Да. Работа распределяется по всем ядрам процессора. Превью показывает первые файлы, а инспектор — до 500 результатов на выход.

## Участие

Issues и pull requests приветствуются. Новая нода — это две части: запись в `src/js/nodes.js` (сокеты и виджеты) и обработчик в `src-tauri/src/ops/`, зарегистрированный в `ops/mod.rs`. CI проверяет, что обе стороны совпадают.

## Лицензия

[MIT](LICENSE) © Morph contributors
