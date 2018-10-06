use std::io::Read;

#[derive(Debug, Clone, Deserialize)]
pub struct CommandInfo {
    http_path: String,
    description: String,
    command: String,
    args: Vec<String>,
}

impl CommandInfo {
    pub fn http_path(&self) -> &String {
        &self.http_path
    }

    pub fn description(&self) -> &String {
        &self.description
    }

    pub fn command(&self) -> &String {
        &self.command
    }

    pub fn args(&self) -> &Vec<String> {
        &self.args
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyInfo {
    http_path: String,
    description: String,
    url: String,
}

impl ProxyInfo {
    pub fn http_path(&self) -> &String {
        &self.http_path
    }

    pub fn description(&self) -> &String {
        &self.description
    }

    pub fn url(&self) -> &String {
        &self.url
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaticPathInfo {
    http_path: String,
    fs_path: String,
    content_type: String,
    cache_control: String,
    include_in_main_page: bool,
}

impl StaticPathInfo {
    pub fn http_path(&self) -> &String {
        &self.http_path
    }

    pub fn fs_path(&self) -> &String {
        &self.fs_path
    }

    pub fn content_type(&self) -> &String {
        &self.content_type
    }

    pub fn cache_control(&self) -> &String {
        &self.cache_control
    }

    pub fn include_in_main_page(&self) -> bool {
        self.include_in_main_page
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MainPageInfo {
    title: String,
}

impl MainPageInfo {
    pub fn title(&self) -> &String {
        &self.title
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Configuration {
    listen_address: String,
    main_page_info: MainPageInfo,
    commands: Vec<CommandInfo>,
    proxies: Vec<ProxyInfo>,
    static_paths: Vec<StaticPathInfo>,
}

impl Configuration {
    pub fn listen_address(&self) -> &String {
        &self.listen_address
    }

    pub fn main_page_info(&self) -> &MainPageInfo {
        &self.main_page_info
    }

    pub fn commands(&self) -> &Vec<CommandInfo> {
        &self.commands
    }

    pub fn proxies(&self) -> &Vec<ProxyInfo> {
        &self.proxies
    }

    pub fn static_paths(&self) -> &Vec<StaticPathInfo> {
        &self.static_paths
    }
}

pub fn read_config(config_file: String) -> Result<Configuration, Box<::std::error::Error>> {
    info!("reading {}", config_file);

    let mut file = ::std::fs::File::open(config_file)?;

    let mut file_contents = String::new();

    file.read_to_string(&mut file_contents)?;

    let configuration: Configuration = ::serde_yaml::from_str(&file_contents)?;

    Ok(configuration)
}
