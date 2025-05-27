use std::{
    time::Instant,
    mem,
    io::{
        self,
        Write,
    },
    fs,
    fs::{
        read_dir,
        copy,
    },
    path::{
        Path,
        PathBuf
    },
    process::Command
};
pub use tempfile::{
    tempdir,
    NamedTempFile,
    Builder,
};
use itertools::Itertools;
use crate::utils::{
    error,
};

pub fn time_it<F>(func: F) -> crate::Result<u128>
where
    F: Fn() -> crate::Result<()>
{
    let before = Instant::now();
    func()?;
    Ok(before.elapsed().as_millis())

}

pub fn shell_spawn(command_list: &[&str]) -> Result<String,String> {
    let command = command_list[0];
    // 创建 Command 实例并运行命令
    let output = Command::new(command)
        .args(&command_list[1..])
        .output()
        .map_err(|e| {
            println!("Failed to execute command '{}': {}", command, e);
            "".to_string()
        })?;

    // 检查命令的退出状态
    if !output.status.success() {
        println!("{}", format!(
            "Command '{}' failed with exit code: {:?}, output: ",
            command_list.iter().join(" "),
            output.status.code()
        ) + &String::from_utf8_lossy(&output.stderr) + &String::from_utf8_lossy(&output.stdout));
        Err("".into())
    }
    else {
        // 将 stdout 转换为 String
        Ok(String::from_utf8_lossy(&output.stderr).into_owned() + &String::from_utf8_lossy(&output.stdout))
    }
}

pub fn setup_test_git_dir() -> tempfile::TempDir {
    let temp_dir = tempdir().unwrap();
    let _ = shell_spawn(&["git", "-C", temp_dir.path().to_str().unwrap(), "init"]).unwrap();
    let _ = shell_spawn(&["git", "-C", temp_dir.path().to_str().unwrap(), "config", "user.name", "rust-git"]).unwrap();
    let _ = shell_spawn(&["git", "-C", temp_dir.path().to_str().unwrap(), "config", "user.email", "163@163.com"]).unwrap();
    let project_root = env!("CARGO_MANIFEST_DIR");
    std::env::set_current_dir(project_root).unwrap();
    temp_dir
}


pub fn mktemp_in<T>(dir: T) -> std::io::Result<PathBuf>
where T: AsRef<Path>
{
    let tempfile = touch_file_in(dir)?;
    let (_, pathbuf) = tempfile.keep()?;
    Ok(pathbuf)
}


pub fn touch_file_in<T>(dir: T) -> std::io::Result<NamedTempFile>
where T: AsRef<Path>
{
    // 指定目录路径
    let dir_path = dir;
    std::fs::create_dir_all(&dir_path)?;

    // 使用 Builder 创建临时文件
    let temp_file = Builder::new()
        .prefix("temp_") // 可选：为文件名添加前缀
        .suffix(".txt")  // 可选：为文件名添加后缀
        .rand_bytes(10)   // 随机字节数（默认是 6 字节）
        .tempfile_in(dir_path)?; // 在指定目录中创建临时文件

    Ok(temp_file)
}

pub fn cp_dir<T>(from: T, to: T) -> Result<String, String>
where
    T: AsRef<Path>
{
    let mut from = from.as_ref().to_path_buf();
    from.push(".");
    let _ = shell_spawn(&["cp", "-a", from.to_str().unwrap(), to.as_ref().to_str().unwrap()]).unwrap();
    Ok("".into())
}

pub type Args<'a> = &'a[&'a str];
pub type ArgsList<'a> = &'a[(Args<'a>, bool)];
pub fn cmd_seq<'a, 'b>(args_list: ArgsList<'a>) -> impl FnMut(Args<'b>) -> Result<Vec<String>, String>
{
    move |command: Args| {
        let command = command.iter().collect::<Vec<_>>();
        args_list.iter()
            .map(|(x, is_print)| {
                let full_cmd = command
                .clone()
                .into_iter().copied()
                .chain(x.iter().copied())
                .collect::<Vec<_>>();
                (full_cmd, is_print)
            })
            .map(|(cmd, is_print)| {
                let output = shell_spawn(cmd.as_slice());
                if *is_print {
                    println!("cmd: {} output: {:?}", cmd.join(" "), output);
                }
                output
            })
            .collect::<Result<Vec<String>, _>>()
    }
}

pub fn run_both<'a>(cmds: ArgsList<'a>, git: Args, cargo: Args) -> Result<(Vec<String>, Vec<String>), String> {
    let mut opers = cmd_seq(cmds);
    Ok((opers(git)?, opers(cargo)?))
}
