<div align="center">
  <img src="https://gitlab.com/mentalgrp/mental.promo/-/raw/main/Logos/constructor_1280x800.png?ref_type=heads" width="400"/>
</div>

# Constructor

**Constructor** — кроссплатформенная система сборки C/C++ приложений, написанная на Rust. Позволяет удобно описывать сборку проектов через простой конфиг-файл и поддерживает параллельную сборку, кеширование, пользовательские скрипты и многое другое.

---

## 🚀 Возможности

- Параллельная сборка нескольких таргетов
- Кеширование сборки (не пересобирает, если ничего не изменилось)
- Асинхронная загрузка git-зависимостей
- Пользовательские pre/post build-скрипты
- Гибкая настройка через TOML-конфиг
- Поддержка переменных окружения, кастомных директорий, дополнительных шагов
- Простое описание зависимостей (git, local, system)

---

## 📦 Пример WORKSPACE файла

```toml
[project]
name = "example"
version = "0.1.0"
language = "C++"
# env = [["GLOBAL_VAR", "value"]]           # (опционально) глобальные переменные окружения
# description = "Example project with all flags" # (опционально) описание проекта

[[dependencies]]
name = "fmt"
source = "git"
location = "https://github.com/fmtlib/fmt.git"

[[targets]]
name = "hell"
out_dir = "bin"
os_target = "macos"
compiler = "clang++"
kind = "executable"
sources = ["src/main.cpp"]
includes = ["deps/fmt/include"]
defines = ["DEBUG=1"]
compiler_flags = ["-std=c++17", "-Wall", "-Wextra"]
linker_flags = ["-Ldeps/fmt/build", "-lfmt"]
pre_build_scripts = [
  "cd deps/fmt && cmake -B build && cmake --build build"
]
post_build_scripts = [
  "echo Build complete!"
]
env = [["MY_VAR", "123"]]                    # (опционально) переменные окружения для таргета
working_dir = "src"                           # (опционально) рабочая директория
custom_output = "bin/custom_hell.out"         # (опционально) кастомный путь для выходного файла
extra_steps = ["echo Extra step"]             # (опционально) дополнительные шаги
enabled = true                                 # (опционально) включён ли таргет
# description = "Main executable with all flags enabled" # (опционально) описание таргета
```

---

## ⚡️ Быстрый старт

1. **Установите Rust** (если ещё не установлен):
   https://www.rust-lang.org/tools/install

2. **Соберите Constructor:**
   ```sh
   cargo build --release
   ```

3. **Создайте WORKSPACE файл** (пример выше)

4. **Запустите сборку:**
   ```sh
   ./target/release/constructor --config WORKSPACE_example.toml
   ```

5. **Очистить кеш и артефакты:**
   ```sh
   ./target/release/constructor --clean --config WORKSPACE_example.toml
   ```

---

## 📚 Документация и поддержка

- [Пример WORKSPACE](./WORKSPACE_example.toml)
- [Rust](https://www.rust-lang.org/)
- Вопросы и предложения: issues или [mentalgrp@protonmail.com](mailto:mentalgrp@protonmail.com)

---

<div align="center">
  <b>Constructor — просто, быстро, кроссплатформенно!</b>
</div>
