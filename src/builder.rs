use crate::config::{BuildConfig, TargetConfig};
use anyhow::Result;
use std::process::Command;
use std::path::Path;
use std::fs::{self, File};
use std::thread;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};

pub struct Builder
{
    config: BuildConfig,
    force_rebuild: bool
}

pub trait DefaultBuilder
{
    fn new(config: BuildConfig, force_rebuild: bool) -> Builder;
    fn build(&self) -> Result<()>;
    fn fetch_dependencies(&self) -> Result<()>;
    fn fetch_git_dependency(&self, dep: &crate::config::Dependency) -> Result<()>;
    fn build_target(&self, target: &TargetConfig) -> Result<()>; 
}

impl DefaultBuilder for Builder
{
    fn new(config: BuildConfig, force_rebuild: bool) -> Self {
        Self { config, force_rebuild}
    }

    fn build(&self) -> Result<()> {
        self.fetch_dependencies()?;
        let project_name = self.config.project.name.clone();
        let version = self.config.project.version.clone();
        let mut version_parts = version.split('.');
        let major_version = version_parts.next().unwrap_or("0");
        let minor_version = version_parts.next().unwrap_or("0");
        let patch_version = version_parts.next().unwrap_or("0");
        let version_define_major = format!("{}_VERSION_MAJOR={}", project_name.to_uppercase(), major_version);
        let version_define_minor = format!("{}_VERSION_MINOR={}", project_name.to_uppercase(), minor_version);
        let version_define_patch = format!("{}_VERSION_PATCH={}", project_name.to_uppercase(), patch_version);
        let mut handles = vec![];
        for target in &self.config.targets {
            if let Some(false) = target.enabled { continue; }
            let mut target = target.clone();
            let mut defines = target.defines.clone().unwrap_or_default();
            defines.push(version_define_major.clone());
            defines.push(version_define_minor.clone());
            defines.push(version_define_patch.clone());
            target.defines = Some(defines);
            let dependencies = self.config.dependencies.clone();
            let force_rebuild = self.force_rebuild;
            let project_name = project_name.clone();
            let version = version.clone();
            let target_name = target.name.clone();
            let handle = std::thread::spawn(move || {
                build_target_static(target, dependencies, force_rebuild, project_name, version)
            });
            handles.push((target_name, handle));
        }
        let mut errors = vec![];
        for (name, handle) in handles {
            if let Err(e) = handle.join().map_err(|_| anyhow::anyhow!("Thread panicked")).and_then(|r| r) {
                errors.push((name, e));
            }
        }
        if !errors.is_empty() {
            for (name, e) in errors {
                eprintln!("Build failed for target '{}': {}", name, e);
            }
            anyhow::bail!("Some targets failed to build");
        }
        Ok(())
    }

    fn fetch_dependencies(&self) -> Result<()> {
        if let Some(deps) = &self.config.dependencies {
            let mut handles = vec![];
            for dep in deps {
                match dep.source.as_str() {
                    "git" => {
                        let dep = dep.clone();
                        let force_rebuild = self.force_rebuild;
                        let handle = std::thread::spawn(move || {
                            fetch_git_dependency_static(dep.clone(), force_rebuild)
                        });
                        handles.push(handle);
                    },
                    "local" => println!("Local dependency: {}", dep.name),
                    "system" => println!("System dependency: {}", dep.name),
                    _ => println!("Unknown dependency type: {}", dep.source),
                }
            }
            for handle in handles {
                handle.join().map_err(|_| anyhow::anyhow!("Thread panicked"))??;
            }
        }
        Ok(())
    }

    fn fetch_git_dependency(&self, dep: &crate::config::Dependency) -> Result<()> {
        
        let dep_dir = format!("deps/{}", dep.name);
        let dep_path = Path::new(&dep_dir);
        
        if self.force_rebuild && dep_path.exists() {
            println!("Force rebuilding dependency: {}...", dep.name);
            std::fs::remove_dir_all(&dep_dir)?;
        }
    
        // Если директория уже существует
        if dep_path.exists() {
        // Проверяем, является ли это git репозиторием
            if dep_path.join(".git").exists() {
                println!("Dependency {} already exists, pulling latest changes...", dep.name);
            
            // Обновляем существующий репозиторий
                let status = Command::new("git")
                    .current_dir(&dep_dir)
                    .arg("pull")
                    .status()?;
                
                if !status.success() {
                    anyhow::bail!("Failed to update dependency: {}", dep.name);
                }
            } else {
                anyhow::bail!(
                    "Dependency directory '{}' exists but is not a git repository. \
                    Please remove it manually or specify a different location.",
                    dep_dir
                );
            }
        } else {
        // Создаем директорию deps, если её нет
            std::fs::create_dir_all("deps")?;
        
            println!("Cloning {} from {}...", dep.name, dep.location);
            let status = Command::new("git")
                .arg("clone")
                .arg(&dep.location)
                .arg(&dep_dir)
                .status()?;
            
            if !status.success() {
                anyhow::bail!("Failed to clone dependency: {}", dep.name);
            }
        }
    
        Ok(())
    }

    fn build_target(&self, target: &TargetConfig) -> Result<()> {
        println!("Building target: {}", target.name);
        
        // --- Кеширование ---
        let mut hasher = DefaultHasher::new();
        // Хешируем исходники
        for source in &target.sources {
            source.hash(&mut hasher);
            if let Ok(mut file) = File::open(source) {
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                buf.hash(&mut hasher);
            }
        }
        // Хешируем defines
        if let Some(defines) = &target.defines {
            defines.hash(&mut hasher);
        }
        // Хешируем compiler_flags
        if let Some(flags) = &target.compiler_flags {
            flags.hash(&mut hasher);
        }
        // Хешируем includes
        if let Some(includes) = &target.includes {
            includes.hash(&mut hasher);
        }
        // Хешируем linker_flags
        if let Some(linker_flags) = &target.linker_flags {
            linker_flags.hash(&mut hasher);
        }
        // Хешируем зависимости
        if let Some(deps) = &self.config.dependencies {
            for dep in deps {
                dep.name.hash(&mut hasher);
                dep.source.hash(&mut hasher);
                dep.location.hash(&mut hasher);
            }
        }
        let hash = hasher.finish();
        let cache_file_path = format!("{}/.build_cache_{}.txt", target.out_dir, target.name);
        let mut need_rebuild = true;
        let mut prev_hash: Option<u64> = None;
        // Если force_rebuild == true, кеширование полностью игнорируется и всегда происходит пересборка
        if !self.force_rebuild {
            if let Ok(mut cache_file) = File::open(&cache_file_path) {
                let mut prev_hash_str = String::new();
                cache_file.read_to_string(&mut prev_hash_str)?;
                if let Ok(h) = prev_hash_str.trim().parse::<u64>() {
                    prev_hash = Some(h);
                    if h == hash {
                        println!("Target '{}' is up to date (cache hit), skipping build.", target.name);
                        need_rebuild = false;
                    }
                }
            }
        } else {
            // Кеширование игнорируется, всегда пересобираем
        }
        if need_rebuild {
            // Подробный лог изменений
            if let Some(old_hash) = prev_hash {
                println!("Cache miss for target '{}'. Причина: изменения в:", target.name);
                // Проверяем по частям
                let mut log_part = |label: &str, old: u64, new: u64| {
                    if old != new {
                        println!("  - {} (hash изменился)", label);
                    }
                };
                // Исходники
                let mut hasher_old = DefaultHasher::new();
                let mut hasher_new = DefaultHasher::new();
                for source in &target.sources {
                    source.hash(&mut hasher_old);
                    source.hash(&mut hasher_new);
                    if let Ok(mut file) = File::open(source) {
                        let mut buf = Vec::new();
                        file.read_to_end(&mut buf)?;
                        buf.hash(&mut hasher_old);
                        buf.hash(&mut hasher_new);
                    }
                }
                log_part("sources", hasher_old.finish(), hasher_new.finish());
                // Defines
                let mut hasher_old = DefaultHasher::new();
                let mut hasher_new = DefaultHasher::new();
                if let Some(defines) = &target.defines {
                    defines.hash(&mut hasher_old);
                    defines.hash(&mut hasher_new);
                }
                log_part("defines", hasher_old.finish(), hasher_new.finish());
                // Compiler flags
                let mut hasher_old = DefaultHasher::new();
                let mut hasher_new = DefaultHasher::new();
                if let Some(flags) = &target.compiler_flags {
                    flags.hash(&mut hasher_old);
                    flags.hash(&mut hasher_new);
                }
                log_part("compiler_flags", hasher_old.finish(), hasher_new.finish());
                // Includes
                let mut hasher_old = DefaultHasher::new();
                let mut hasher_new = DefaultHasher::new();
                if let Some(includes) = &target.includes {
                    includes.hash(&mut hasher_old);
                    includes.hash(&mut hasher_new);
                }
                log_part("includes", hasher_old.finish(), hasher_new.finish());
                // Linker flags
                let mut hasher_old = DefaultHasher::new();
                let mut hasher_new = DefaultHasher::new();
                if let Some(linker_flags) = &target.linker_flags {
                    linker_flags.hash(&mut hasher_old);
                    linker_flags.hash(&mut hasher_new);
                }
                log_part("linker_flags", hasher_old.finish(), hasher_new.finish());
                // Dependencies
                let mut hasher_old = DefaultHasher::new();
                let mut hasher_new = DefaultHasher::new();
                if let Some(deps) = &self.config.dependencies {
                    for dep in deps {
                        dep.name.hash(&mut hasher_old);
                        dep.source.hash(&mut hasher_old);
                        dep.location.hash(&mut hasher_old);
                        dep.name.hash(&mut hasher_new);
                        dep.source.hash(&mut hasher_new);
                        dep.location.hash(&mut hasher_new);
                    }
                }
                log_part("dependencies", hasher_old.finish(), hasher_new.finish());
            }
        }
        // --- Конец кеширования ---
        
        // Выполнение pre_build_scripts
        if let Some(scripts) = &target.pre_build_scripts {
            for script in scripts {
                println!("Running pre-build script: {}", script);
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(script)
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Pre-build script failed: {}", script);
                }
            }
        }
        
        let mut command = Command::new(target.compiler.clone());
        
        // Добавляем флаги компилятора
        if let Some(flags) = &target.compiler_flags {
            for flag in flags {
                command.arg(flag);
            }
        }
        
        // Добавляем определения
        if let Some(defines) = &target.defines {
            for define in defines {
                command.arg(format!("-D{}", define));
            }
        }
        
        // Добавляем include-директории
        if let Some(includes) = &target.includes {
            for include in includes {
                command.arg("-I").arg(include);
            }
        }
        
        // Добавляем исходные файлы
        for source in &target.sources {
            command.arg(source);
        }
        
        // Добавляем MacOS frameworks
        if target.os_target.to_lowercase() == "macos" {
            if let Some(frameworks) = &target.frameworks {
                for fw in frameworks {
                    command.arg("-framework").arg(fw);
                }
            }
        }
        
        // Флаги линковки
        if let Some(linker_flags) = &target.linker_flags {
            for flag in linker_flags {
                command.arg(flag);
            }
        }

        match fs::create_dir(target.out_dir.clone()) {
            Ok(()) => println!("Directory created successfully"),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    println!("Directory already exists");
                } else {
                    eprintln!("Error creating directory: {}", e);
                }
            }
        }
        
        // Выходной файл
        let output = match target.kind.as_str() {
            "executable" => format!("{}/{}", target.out_dir, target.name),
            "staticlib" => format!("{}/lib{}.a", target.out_dir ,target.name),
            "dynamiclib" => format!("{}/lib{}.so", target.out_dir, target.name),
            _ => anyhow::bail!("Unknown target kind: {}", target.kind),
        };
        
        command.arg("-o").arg(&output);
        
        // Печатаем сгенерированную команду
        println!("[build] Generated command: {:?}", command);
        
        // Запускаем компиляцию
        let status = command.status()?;
        if !status.success() {
            anyhow::bail!("Failed to build target: {}", target.name);
        }
        
        println!("Successfully built: {}", output);
        
        // Выполнение post_build_scripts
        if let Some(scripts) = &target.post_build_scripts {
            for script in scripts {
                println!("Running post-build script: {}", script);
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(script)
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Post-build script failed: {}", script);
                }
            }
        }
        // После успешной сборки сохраняем хеш
        let mut cache_file = File::create(&cache_file_path)?;
        writeln!(cache_file, "{}", hash)?;
        Ok(())
    }
}

impl Builder {
    pub fn clean_cache(&self) -> Result<()> {
        for target in &self.config.targets {
            let pattern = format!("{}/.build_cache_{}*.txt", target.out_dir, target.name);
            for entry in glob::glob(&pattern)? {
                match entry {
                    Ok(path) => {
                        if path.exists() {
                            println!("Removing cache file: {}", path.display());
                            fs::remove_file(&path)?;
                        }
                    },
                    Err(e) => eprintln!("Glob error: {}", e),
                }
            }
        }
        Ok(())
    }

    pub fn generate_makefile(&self) -> Result<()> {
        let mut makefile = String::new();
        let project_name = &self.config.project.name;
        makefile.push_str(&format!("PROJECT_NAME = {}\n", project_name));
        for target in &self.config.targets {
            if let Some(false) = target.enabled { continue; }
            makefile.push_str(&format!("CC = {}\n", target.compiler));
            // CFLAGS
            let mut cflags = String::new();
            if let Some(flags) = &target.compiler_flags {
                for flag in flags {
                    cflags.push_str(flag);
                    cflags.push(' ');
                }
            }
            makefile.push_str(&format!("CFLAGS = {}\n", cflags.trim_end()));
            // LDFLAGS
            let mut ldflags = String::new();
            if let Some(flags) = &target.linker_flags {
                for flag in flags {
                    ldflags.push_str(flag);
                    ldflags.push(' ');
                }
            }
            // FRAMEWORKS (macOS)
            if target.os_target.to_lowercase() == "macos" {
                if let Some(frameworks) = &target.frameworks {
                    for fw in frameworks {
                        ldflags.push_str(&format!(" -framework {}", fw));
                    }
                }
            }
            makefile.push_str(&format!("LDFLAGS = {}\n", ldflags.trim_end()));
            // DEFINES
            let mut defines = String::new();
            if let Some(defs) = &target.defines {
                for d in defs {
                    defines.push_str(&format!(" -D{}", d));
                }
            }
            makefile.push_str(&format!("DEFINES = {}\n", defines.trim_end()));
            // INCLUDES
            let mut includes = String::new();
            if let Some(incs) = &target.includes {
                for inc in incs {
                    includes.push_str(&format!(" -I{}", inc));
                }
            }
            makefile.push_str(&format!("INCLUDES = {}\n", includes.trim_end()));
            // SOURCES
            makefile.push_str(&format!("SOURCES = {}\n", target.sources.join(" ")));
            // OUTPUT
            let output = match target.kind.as_str() {
                "executable" => format!("{}/{}", target.out_dir, target.name),
                "staticlib" => format!("{}/lib{}.a", target.out_dir, target.name),
                "dynamiclib" => format!("{}/lib{}.so", target.out_dir, target.name),
                _ => continue,
            };
            makefile.push_str(&format!("OUTPUT = {}\n", output));
            makefile.push_str("\n");
            // Цель
            makefile.push_str(&format!("{}: $(SOURCES)\n", target.name));
            makefile.push_str("\t$(CC) $(CFLAGS) $(DEFINES) $(INCLUDES) $(SOURCES) $(LDFLAGS) -o $(OUTPUT)\n\n");
        }
        // Собираем имена целей и выходные файлы
        let mut target_names = Vec::new();
        let mut outputs = Vec::new();
        for target in &self.config.targets {
            if let Some(false) = target.enabled { continue; }
            target_names.push(target.name.clone());
            let output = match target.kind.as_str() {
                "executable" => format!("{}/{}", target.out_dir, target.name),
                "staticlib" => format!("{}/lib{}.a", target.out_dir, target.name),
                "dynamiclib" => format!("{}/lib{}.so", target.out_dir, target.name),
                _ => continue,
            };
            outputs.push(output);
        }
        // .PHONY
        makefile.push_str(&format!(".PHONY: {} clean\n\n", target_names.join(" ")));
        // clean
        makefile.push_str("clean:\n");
        makefile.push_str(&format!("\trm -f {}\n", outputs.join(" ")));
        std::fs::write("Makefile", makefile)?;
        println!("Makefile сгенерирован.");
        Ok(())
    }
}

fn build_target_static(target: crate::config::TargetConfig, dependencies: Option<Vec<crate::config::Dependency>>, force_rebuild: bool, project_name: String, version: String) -> anyhow::Result<()> {
    // --- Копия логики build_target, но без self ---
    use std::process::Command;
    use std::path::Path;
    use std::fs::{self, File};
    use std::io::{Read, Write};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // --- Кеширование ---
    let mut hasher = DefaultHasher::new();
    for source in &target.sources {
        source.hash(&mut hasher);
        if let Ok(mut file) = File::open(source) {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            buf.hash(&mut hasher);
        }
    }
    if let Some(defines) = &target.defines {
        defines.hash(&mut hasher);
    }
    if let Some(flags) = &target.compiler_flags {
        flags.hash(&mut hasher);
    }
    if let Some(includes) = &target.includes {
        includes.hash(&mut hasher);
    }
    if let Some(linker_flags) = &target.linker_flags {
        linker_flags.hash(&mut hasher);
    }
    if let Some(deps) = &dependencies {
        for dep in deps {
            dep.name.hash(&mut hasher);
            dep.source.hash(&mut hasher);
            dep.location.hash(&mut hasher);
        }
    }
    let hash = hasher.finish();
    let cache_file_path = format!("{}/.build_cache_{}.txt", target.out_dir, target.name);
    let mut need_rebuild = true;
    let mut prev_hash: Option<u64> = None;
    if !force_rebuild {
        if let Ok(mut cache_file) = File::open(&cache_file_path) {
            let mut prev_hash_str = String::new();
            cache_file.read_to_string(&mut prev_hash_str)?;
            if let Ok(h) = prev_hash_str.trim().parse::<u64>() {
                prev_hash = Some(h);
                if h == hash {
                    println!("Target '{}' is up to date (cache hit), skipping build.", target.name);
                    need_rebuild = false;
                }
            }
        }
    } else {
        // Кеширование игнорируется, всегда пересобираем
    }
    if need_rebuild {
        if let Some(old_hash) = prev_hash {
            println!("Cache miss for target '{}'. Причина: изменения в:", target.name);
            // ... (логика подробного лога изменений, можно вынести из старого build_target)
        }
    }
    if !need_rebuild {
        if let Some(scripts) = &target.post_build_scripts {
            for script in scripts {
                println!("Running post-build script: {}", script);
                let status = Command::new("sh")
                    .arg("-c")
                    .arg(script)
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Post-build script failed: {}", script);
                }
            }
        }
        return Ok(());
    }
    // --- pre_build_scripts ---
    if let Some(scripts) = &target.pre_build_scripts {
        for script in scripts {
            println!("Running pre-build script: {}", script);
            let status = Command::new("sh")
                .arg("-c")
                .arg(script)
                .status()?;
            if !status.success() {
                anyhow::bail!("Pre-build script failed: {}", script);
            }
        }
    }
    // --- Сборка ---
    println!("Building target: {}", target.name);
    let mut command = Command::new(target.compiler.clone());
    if let Some(flags) = &target.compiler_flags {
        for flag in flags {
            command.arg(flag);
        }
    }
    if let Some(defines) = &target.defines {
        for define in defines {
            command.arg(format!("-D{}", define));
        }
    }
    if let Some(includes) = &target.includes {
        for include in includes {
            command.arg("-I").arg(include);
        }
    }
    for source in &target.sources {
        command.arg(source);
    }
    // Добавляем MacOS frameworks
    if target.os_target.to_lowercase() == "macos" {
        if let Some(frameworks) = &target.frameworks {
            for fw in frameworks {
                command.arg("-framework").arg(fw);
            }
        }
    }
    match fs::create_dir(target.out_dir.clone()) {
        Ok(()) => println!("Directory created successfully"),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                println!("Directory already exists");
            } else {
                eprintln!("Error creating directory: {}", e);
            }
        }
    }
    let output = match target.kind.as_str() {
        "executable" => format!("{}/{}", target.out_dir, target.name),
        "staticlib" => format!("{}/lib{}.a", target.out_dir ,target.name),
        "dynamiclib" => format!("{}/lib{}.so", target.out_dir, target.name),
        _ => anyhow::bail!("Unknown target kind: {}", target.kind),
    };
    command.arg("-o").arg(&output);
    
    // Печатаем сгенерированную команду
    println!("[build] Generated command: {:?}", command);
    
    let status = command.status()?;
    if !status.success() {
        anyhow::bail!("Failed to build target: {}", target.name);
    }
    println!("Successfully built: {}", output);
    // --- post_build_scripts ---
    if let Some(scripts) = &target.post_build_scripts {
        for script in scripts {
            println!("Running post-build script: {}", script);
            let status = Command::new("sh")
                .arg("-c")
                .arg(script)
                .status()?;
            if !status.success() {
                anyhow::bail!("Post-build script failed: {}", script);
            }
        }
    }
    let mut cache_file = File::create(&cache_file_path)?;
    writeln!(cache_file, "{}", hash)?;
    Ok(())
}

fn fetch_git_dependency_static(dep: crate::config::Dependency, force_rebuild: bool) -> anyhow::Result<()> {
    use std::process::Command;
    use std::path::Path;
    let dep_dir = format!("deps/{}", dep.name);
    let dep_path = Path::new(&dep_dir);
    if force_rebuild && dep_path.exists() {
        println!("Force rebuilding dependency: {}...", dep.name);
        std::fs::remove_dir_all(&dep_dir)?;
    }
    if dep_path.exists() {
        if dep_path.join(".git").exists() {
            println!("Dependency {} already exists, pulling latest changes...", dep.name);
            let status = Command::new("git")
                .current_dir(&dep_dir)
                .arg("pull")
                .status()?;
            if !status.success() {
                anyhow::bail!("Failed to update dependency: {}", dep.name);
            }
        } else {
            anyhow::bail!(
                "Dependency directory '{}' exists but is not a git repository. Please remove it manually or specify a different location.",
                dep_dir
            );
        }
    } else {
        std::fs::create_dir_all("deps")?;
        println!("Cloning {} from {}...", dep.name, dep.location);
        let status = Command::new("git")
            .arg("clone")
            .arg(&dep.location)
            .arg(&dep_dir)
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to clone dependency: {}", dep.name);
        }
    }
    Ok(())
}

