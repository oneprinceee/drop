# drop

[english](#english) • [русский](#русский)

---

## english

ultra lightweight and fully open-source installer engine for windows written in rust

### features

- fully open-source
- ultra lightweight (source code is ~19 kb)
- native win32 ui (`native-windows-gui`), no electron or chromium
- strong payload compression using `zstd` and `tar`
- single standalone `.exe` with embedded assets
- no code edits required, configured via `drop.toml`
- supports `%appdata%`, `%localappdata%`, `%programfiles%` paths
- automatic desktop and start menu shortcuts
- built-in uninstaller (`unins000.exe` / `--uninstall`)
- saves window position across runs
- custom post-install and pre-uninstall scripts

### how to use

1. download repository (Code -> Download ZIP or `git clone`)
2. put your files into `payload/`
3. configure `drop.toml`
4. run `build.bat` (output will be in `dist/`)

---

## русский

полностью открытый и ультра-легкий движок инсталляторов для windows на rust

### возможности

- полностью открытый исходный код
- ультра-легкий (весь исходный код ~19 кб)
- нативный win32 интерфейс (`native-windows-gui`), без электрона и хромиума
- сильное сжатие файлов через `zstd` и `tar`
- один готовый `.exe` со всеми ресурсами внутри
- настраивается через `drop.toml`, код менять не нужно
- поддержка системных путей (`%appdata%`, `%localappdata%`, `%programfiles%`)
- создание ярлыков на рабочем столе и в меню пуск
- встроенный удалятор (`unins000.exe` / `--uninstall`)
- запоминает положение окна при перезапуске
- запуск своих скриптов после установки и перед удалением

### как использовать

1. скачайте репозиторий (Code -> Download ZIP или `git clone`)
2. закиньте файлы своей программы в `payload/`
3. настройте `drop.toml`
4. запустите `build.bat` (готовый установщик появится в `dist/`)
