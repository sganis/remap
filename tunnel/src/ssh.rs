use std::io::{Read, Write};
use std::net::{TcpStream, TcpListener, SocketAddr, ToSocketAddrs};
use socket2::{Socket, Domain, Type};
use std::time::Duration;
use ssh2::{Session, Sftp, FileStat};
use std::path::{PathBuf, Path};
use super::command;

#[derive(Default)]
pub struct Ssh {
    session : Option<Session>,
    sftp : Option<Sftp>,
    host : String,
    user : String,
    password : String,
    private_key : String,
}

#[derive(Clone, serde::Serialize)]
struct Payload {
    percent: f32,
}

#[allow(dead_code)]
impl Ssh {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    
    pub fn supported_algs() -> String {
        let ssh = Session::new().unwrap();
        println!("hostKey: {:?}", ssh.supported_algs(ssh2::MethodType::HostKey).unwrap());
        println!("CryptCs: {:?}", ssh.supported_algs(ssh2::MethodType::CryptCs).unwrap());
        println!("Kex: {:?}", ssh.supported_algs(ssh2::MethodType::Kex).unwrap());
        println!("MacCs: {:?}", ssh.supported_algs(ssh2::MethodType::MacCs).unwrap());
        println!("CompCs: {:?}", ssh.supported_algs(ssh2::MethodType::CompCs).unwrap());

        "supported flags above".to_string()
    }
    pub fn private_key_path() -> PathBuf {
        let home = dirs::home_dir().unwrap();
        let prikey = home.join(".ssh").join("id_rsa");
        PathBuf::from(&prikey)
        
    }
    pub fn public_key_path() -> PathBuf {
        let home = dirs::home_dir().unwrap();
        let pubkey = home.join(".ssh").join("id_rsa.pub").clone();
        PathBuf::from(&pubkey)
    }    
    pub fn has_private_key() -> bool {     
        Ssh::private_key_path().exists()
        
    }
    pub fn has_public_key() -> bool {      
        Ssh::public_key_path().exists()
    }
    fn generate_public_key() -> Result<(), String> {
        let seckey = Ssh::private_key_path();
        let pubkey = Ssh::public_key_path();
        
        let cmd = format!("ssh-keygen -f {} -y > {}", seckey.display(), pubkey.display());
        let (_,e,_) = command::run(&cmd);
        
        if e.len()>0 {
            Err(e)
        } else {
            Ok(())
        }
    }
    fn generate_keys() -> Result<(), String> {
        let seckey = Ssh::private_key_path();
        
        let cmd = format!("ssh-keygen -m PEM -N \"\" -f {}", seckey.display());
        let (_,e,_) = command::run(&cmd);
        
        if e.len()>0 {
            Err(e)
        } else {
            Ok(())
        }
    }
    fn transfer_public_key(host: &str, port: i16, user: &str, password: &str) -> Result<(), String> {
        let pubkeytext = std::fs::read_to_string(&Ssh::public_key_path()).unwrap().trim().to_string();
        let cmd = format!("exec sh -c \"cd; umask 077; mkdir -p .ssh; echo '{}' >> .ssh/authorized_keys\"",
                        pubkeytext);
        println!("{cmd}");
        let mut ssh = Ssh::new();
        if let Err(e) = ssh.connect_with_password(host, port, user, password) {
            println!("Error transfering keys, login with password: {e}");
            return Err(e);
        }
        if let Err(e) = ssh.run(&cmd) {
            println!("Error transfering keys, running command: {e}");            
            Err(e)
        } else {
            Ok(())
        }

    }
    fn test_ssh(host: &str, port: i16, user: &str) -> Result<(), String> {
        if !Ssh::has_private_key() {
            return Err("No private key".to_string());
        }
        let pkey = Ssh::private_key_path();
        let mut ssh = Ssh::new();
        if let Err(e) = ssh.connect_with_key(host, port, user, pkey.to_str().unwrap()) {
            Err(e)
        } else {        
            Ok(())
        }
    }
    pub fn setup_ssh(host: &str, port: i16, user: &str, password: &str) -> Result<(), String> {
        if !Ssh::has_private_key() {
            if let Err(e) = Ssh::generate_keys() {
                return Err(format!("Could not generate private key: {e}"));
            }
        }
        if !Ssh::has_public_key() {
            if let Err(e) = Ssh::generate_public_key() {
                return Err(format!("Could not generate public key: {e}"));
            }         
        }
        if Ssh::test_ssh(host, port, user).is_err() {
            if let Err(e) = Ssh::transfer_public_key(host, port, user, password) {
                return Err(format!("Could not transfer public key: {e}"));
            }
            if let Err(e) = Ssh::test_ssh(host, port, user) {
                return Err(format!("Test ssh failed: {e}"));
            }
        }
        Ok(())
    }
    fn _get_tcp(&mut self, host: &str, port: i16) -> Result<TcpStream, String> {
        let timeout = Duration::new(5, 0); // 5 secs
        let addresses: Vec<_> = match format!("{}:{}", host, port).to_socket_addrs() {
            Err(e) => {
                println!("Unable to resolve address: {}:{}  {:?}",host, port, e);
                return Err(e.to_string())
            },
            Ok(o) => o.collect(),
        };
        let mut tcp = None;
        let mut error = String::new();

        for addr in addresses {
            match TcpStream::connect_timeout(&addr, timeout) {
                Err(e) => {
                    //error.push_str(&format!("tcp error: {:?}\n", e));
                    error = String::from(&format!("tcp error: {:?}", e));
                    continue;
                },
                Ok(o) => {
                    println!("connected to: {:?}", addr);
                    tcp = Some(o);
                    break;  
                },
            };
        }

        if tcp.is_none() {
            return Err(error);
        }

        Ok(tcp.unwrap())
    }
    pub fn connect_with_password(&mut self, 
        host: &str, port: i16, user: &str, password: &str) -> Result<(), String> {
        
        let tcp = match self._get_tcp(host, port) {
            Err(e) => return Err(e),
            Ok(o) => o,
        };

        let mut session = Session::new().unwrap();
        session.set_tcp_stream(tcp);

        if let Err(e) = session.handshake() {                
            return Err(format!("SSH handshake error: {}", e));
        }

        if let Err(e) = session.userauth_password(user, password) {
            return Err(format!("Authentication error: {e}"));
        }

        assert!(session.authenticated());
        let sftp = match session.sftp() {
            Err(e) => return Err(format!("Cannot create sftp channel {e}")),
            Ok(o) => o,
        };

        self.session = Some(session);
        self.sftp = Some(sftp);
        self.host = host.to_string();
        self.user = user.to_string();
        self.password = password.to_string();
        Ok(())
    }
    pub fn connect_with_key(&mut self, 
        host: &str, port: i16, user: &str, pkey: &str) -> Result<(), String> {
        let tcp = match self._get_tcp(host, port) {
            Err(e) => return Err(e),
            Ok(o) => o,
        };
        let mut session = Session::new().unwrap();
        session.set_tcp_stream(tcp);

        if let Err(e) = session.handshake() {
            return Err(format!("SSH handshake error: {}", e));
        }

        let private_key = std::path::Path::new(pkey);

        if let Err(e) = session.userauth_pubkey_file(user, None, private_key, None) {
            return Err(format!("Authentication error: {e}"));
        }

        assert!(session.authenticated());

        let sftp = match session.sftp() {
            Err(e) => return Err(format!("Cannot create sftp channel {e}")),
            Ok(o) => o,
        };

        self.session = Some(session);
        self.sftp = Some(sftp);
        self.host = host.to_string();
        self.user = user.to_string();
        self.private_key = pkey.to_string();
        Ok(())
    }
    pub fn direct_tcpip(&mut self, 
        shost: &str, sport: u16, rhost: &str, rport: u16) -> Result<(), String> {
        
        //let listener = TcpListener::bind(format!("{shost}:{sport}")).unwrap();
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        socket.set_reuse_address(true);
        let address: SocketAddr = format!("{shost}:{sport}").parse().unwrap();
        let address = address.into();
        socket.bind(&address).unwrap();
        socket.listen(2).unwrap();        
        let listener: TcpListener = socket.into();
        
        println!("Waiting for connections in {shost}:{sport}...");
        let mut forwarder = match listener.accept() {
            Ok((socket, addr)) => {
                println!("new client: {addr:?}");
                socket
            },
            Err(e) => return Err(format!("couldn't get client: {e:?}"))
        };

        println!("Forwarding connections from {shost}:{sport} to {rhost}:{rport}...");
        
        let mut channel = match self.session.as_ref().unwrap()
            .channel_direct_tcpip(rhost, rport, Some((shost, sport))) {
            Err(e) => return Err(format!("Error creating channel for direct tcpip: {}", e)),
            Ok(o) => o,
        };  
        let mut buf = [0u8; 3000];
        
        // loop {            
        //     let len = forwarder.read(&mut buf).unwrap();
        //     println!("bytes recieved: {len}");
        //     let mut wr = 0 as usize;
        //     while wr < len {
        //         let i = match channel.write(&buf[wr..]) {
        //             Ok(i) => i,
        //             Err(e) => return Err(format!("channel_write: {:?}",e))
        //         };
        //         wr += i;
        //     }
        //     loop {
        //         let len = match channel.read(&mut buf) {
        //             Ok(len) => len,
        //             Err(e) => {
        //                 println!("channel_write: {:?}",e);
        //                 break;
        //             },
        //         };
        //         wr = 0 as usize;
        //         while wr < len {
        //             let i = match forwarder.write(&buf[wr..]) { 
        //                 Ok(i) => i,
        //                 Err(e) => return Err(format!("forwarder write: {:?}",e)),
        //             };
        //             wr += i;
        //         }
        //         if channel.eof() {
        //             return Err(format!("server disconnected"));
        //         }
        //     }


        // }
        
    

        
        Ok(())
    }
    pub fn disconnect(&mut self) -> Result<(), String> {
        if let Err(e) = self.session.as_ref().unwrap().disconnect(None,"",None) {
            return Err(e.to_string());
        }
        Ok(())
    }
    pub fn run(&mut self, cmd: &str) -> Result<String, String> {
        println!("running CMD: {}", cmd);
        let mut channel = match self.session.as_ref().unwrap().channel_session() {
            Err(e) => return Err(format!("Error: {}", e)),
            Ok(o) => o,
        };
        channel.exec(cmd).unwrap();
        let mut s = String::new();
        channel.stderr().read_to_string(&mut s).unwrap();
        if !s.trim().is_empty() {
            return Err(format!("stderr: {}",s));
        };
        channel.read_to_string(&mut s).unwrap();
        channel.wait_close().unwrap();
        Ok(s.trim().to_string())
    }
    pub fn sftp_stat(&mut self, filename: &str) -> Result<FileStat, String> {
        match self.sftp.as_ref().unwrap().lstat(Path::new(filename)) {
            Err(e) => Err(format!("Cannot stat {filename}: {e}")),
            Ok(o) => Ok(o)
        }
    }
    pub fn sftp_mkdir(&mut self, dirname: &str) -> Result<(), String> {
        match self.sftp.as_ref().unwrap().mkdir(Path::new(dirname), 0o755) {
            Err(e) => Err(format!("Cannot make dir {dirname}: {e}")),
            Ok(_) => Ok(())
        }
    }
    pub fn sftp_rmdir(&mut self, dirname: &str) -> Result<(), String> {
        match self.sftp.as_ref().unwrap().rmdir(Path::new(dirname)) {
            Err(e) => Err(format!("Cannot delete dir {dirname}: {e}")),
            Ok(_) => Ok(())
        }
    }
    pub fn sftp_create(&mut self, filename: &str) -> Result<ssh2::File, String> {
        let f = match self.sftp.as_ref().unwrap().create(Path::new(filename)) {
            Err(e) => return Err(format!("Cannot create file {filename}: {e}")),
            Ok(o) => o,
        };
        Ok(f)
    }
    pub fn sftp_open(&mut self, filename: &str) -> Result<ssh2::File, String> {
        let f = match self.sftp.as_ref().unwrap().open(Path::new(filename)) {
            Err(e) => return Err(format!("Cannot open file {filename}: {e}")),
            Ok(o) => o,
        };
        Ok(f)
    }
    pub fn sftp_rename(&mut self, src: &str, dst: &str) -> Result<(), String> {
        let s = Path::new(src);
        let d = Path::new(dst);
        let sftp = self.sftp.as_ref().unwrap();
        if let Err(e) = sftp.rename(&s, &d, None) {
            return Err(format!("Cannot rename {src}: {e}"));
        }
        Ok(())
    }
    pub fn sftp_delete(&mut self, filename: &str) -> Result<(), String> {
        println!("deleting {filename}");
        
        let path = Path::new(filename);
        let sftp = self.sftp.as_ref().unwrap();
        let stat = match sftp.lstat(path) {
            Err(e) => return Err(format!("{filename}: {e}")),
            Ok(o) => o,
        };
        if stat.file_type().is_symlink() || stat.file_type().is_file() {
            //println!("{filename} is file or link");        
            match sftp.unlink(path) {
                Err(e) => Err(format!("Cannot delete {filename}: {e}")),
                Ok(_) => Ok(())
            }    
        } else {
            //println!("file is folder: {filename}");
            let files: Vec<(PathBuf, FileStat)> = match sftp.readdir(path) {
                Err(e) => return Err(format!("Cannot read directory {filename}: {e}")),
                Ok(o) => o
            };
            //println!("files in: {filename}: {}: {:?}", files.len(), files);                
            if files.len() > 0 {
               for (f,_) in files {
                    if let Err(e) = self.sftp_delete(f.clone().to_str().unwrap()) {
                        return Err(format!("Cannot delete directory {filename}: {e}"));
                    }
                }
            }
            println!("rmdir folder: {filename}");
            match self.sftp.as_ref().unwrap().rmdir(path) {
                Err(e) => return Err(format!("Cannot delete directory {filename}: {e}")),
                Ok(_) => Ok(()),
            }
        }
    }
    pub fn sftp_readdir(&mut self, dirname: &str) 
    -> Result<Vec<(PathBuf, FileStat)>, String> {
        let path = Path::new(dirname);
        let files: Vec<(PathBuf, FileStat)> = match self.sftp.as_ref().unwrap().readdir(path) {
            Err(e) => return Err(format!("Cannot read directory {dirname}: {e}")),
            Ok(o) => o
        };
        Ok(files)
    }
    pub fn sftp_readlink(&mut self, filename: &str) -> Result<String, String> {
        let path = Path::new(filename);
        let destination = match self.sftp.as_ref().unwrap().readlink(path) {
            Err(e) => return Err(format!("Cannot read path {filename}: {e}")),
            Ok(o) => o
        };
        Ok(String::from(destination.to_string_lossy()))
    }
    pub fn sftp_realpath(&mut self, filename: &str) -> Result<(String, FileStat), String> {
        let path = Path::new(filename);
        let sftp = self.sftp.as_ref().unwrap();
        let destination = match self.sftp.as_ref().unwrap().realpath(path) {
            Err(e) => return Err(format!("Cannot read real path {filename}: {e}")),
            Ok(o) => o
        };
        let stat = match sftp.stat(&destination) {
            Err(e) => return Err(format!("Cannot stat {filename}: {e}")),
            Ok(o) => o
        };
        Ok((String::from(destination.to_string_lossy()), stat))
    }
    pub fn sftp_save(&mut self, filename: &str, data: &str) -> Result<(), String> {
        let mut f = match self.sftp.as_ref().unwrap().create(Path::new(filename)) {
            Err(e) => return Err(format!("Cannot create file {filename}: {e}")),
            Ok(o) => o,
        };
        f.write_all(data.as_bytes()).expect("Cannot write data");
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    
    use super::*;
    use std::env;
    const PORT: i16 = 22;

    fn get_params() -> (String, String, String) {
        let host = env::var("TEST_SSH_HOST").unwrap();
        let user = env::var("TEST_SSH_USER").unwrap();
        let pass = env::var("TEST_SSH_PASS").unwrap();
        assert!(host.len()>0);
        assert!(user.len()>0);
        assert!(pass.len()>0);
        (host, user, pass)
    }
    #[test]
    fn connect_with_password() {
        let mut ssh = Ssh::new();
        let (host, user, pass) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass);
        assert!(r.is_ok());
    }
    #[test]
    fn connect_with_password_wrong() {
        let mut ssh = Ssh::new();
        let (host, user, _) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, "wrong");
        assert!(r.is_err());
    }
    #[test]
    fn connect_with_key() {
        let mut ssh = Ssh::new();
        let (host, user, _) = get_params();
        let pkey = Ssh::private_key_path();
        let pkey = pkey.to_str().unwrap();
        let r = ssh.connect_with_key(&host, PORT, &user, &pkey);
        assert!(r.is_ok());
    }
    #[test]
    fn connect_with_key_wrong() {
        let mut ssh = Ssh::new();
        let (host, user, _) = get_params();
        let r = ssh.connect_with_key(&host, PORT, &user, "/invalid/key");
        assert!(r.is_err());
    }
    #[test]
    fn connect_with_host_wrong() {
        let mut ssh = Ssh::new();
        let (_, user, pass) = get_params();
        let r = ssh.connect_with_password("example.com", PORT, &user, &pass);
        assert!(r.is_err());
    }
    #[test]
    fn run_command() {
        let mut ssh = Ssh::new();
        let (host, user, pass) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass);
        assert!(r.is_ok());
        let output = ssh.run("whoami").unwrap();
        assert_eq!("support", output.as_str());
    }
    #[test]
    fn has_private_key() {
        assert!(Ssh::has_private_key());
    }
    #[test]
    #[ignore = "makes ssh keys unavailable for other tests to pass"]
    fn generate_keys() {
        let seckey = Ssh::private_key_path();
        let secbak = PathBuf::from(seckey.to_string_lossy().to_string() + ".bak");
        let pubkey = Ssh::public_key_path();
        let pubbak = PathBuf::from(pubkey.to_string_lossy().to_string() + ".bak");

        // backup keys
        if seckey.exists() {
            std::fs::rename(&seckey, &secbak).unwrap();
        }
        if pubkey.exists() {
            std::fs::rename(&pubkey, &pubbak).unwrap();
        }
        assert!(!Ssh::has_private_key());
        assert!(!Ssh::has_public_key());

        assert!(Ssh::generate_keys().is_ok()); 
        assert!(Ssh::generate_public_key().is_ok());  
        assert!(Ssh::has_private_key());
        assert!(Ssh::has_public_key());

        // restore keys
        if secbak.exists() {
            std::fs::rename(&secbak, &seckey).unwrap();
        }
        if pubbak.exists() {
            std::fs::rename(&pubbak, &pubkey).unwrap();
        }
        
    }
    
    #[test]
    fn setup_ssh() {
        let (host, user, pass) = get_params();
        assert!(Ssh::setup_ssh(&host, PORT, &user, &pass).is_ok());
    }

    #[test]
    fn supported_algs() {
        println!("{}",Ssh::supported_algs());
    }
    #[test]
    fn mkdir_rmdir() {
        let mut ssh = Ssh::new();
        let (host, user, pass) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass);
        assert!(r.is_ok());
        assert!(ssh.sftp_stat( "/home/support").is_ok());
        assert!(ssh.sftp_stat( "/home/support/folder").is_err());
        assert!(ssh.sftp_mkdir("/home/support/folder").is_ok());
        assert!(ssh.sftp_stat( "/home/support/folder").is_ok());
        assert!(ssh.sftp_rmdir("/home/support/folder").is_ok());
        assert!(ssh.sftp_stat( "/home/support/folder").is_err());
        
    }
    #[test]
    fn create_delete() {
        let mut ssh = Ssh::new();
        let (host, user, pass) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass);
        assert!(r.is_ok());
        assert!(ssh.sftp_stat( "/home/support").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file").is_err());
        assert!(ssh.sftp_create("/home/support/file").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file").is_ok());
        assert!(ssh.sftp_delete("/home/support/file").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file").is_err());

        assert!(ssh.sftp_mkdir( "/home/support/dir").is_ok());
        assert!(ssh.sftp_stat( "/home/support/dir").is_ok());
        assert!(ssh.sftp_create("/home/support/dir/file").is_ok());
        assert!(ssh.sftp_stat( "/home/support/dir/file").is_ok());
        assert!(ssh.sftp_delete( "/home/support/dir").is_ok());
        assert!(ssh.sftp_stat( "/home/support/dir").is_err());
        
    }
    #[test]
    fn readdir() {
        let mut ssh = Ssh::new();
        let (host, user, pass) = get_params();        
        let r = ssh.connect_with_password(&host, PORT, &user, &pass);
        assert!(r.is_ok());
        assert!(ssh.sftp_stat( "/home/support").is_ok());
        let files = ssh.sftp_readdir("/").unwrap();
        assert!(files.len() > 0);
        // for f in files  {
        //     println!("{:?}", f);
        // }
    }
    #[test]
    fn rename() {
        let mut ssh = Ssh::new();
        let (host, user, pass) = get_params();
        let r = ssh.connect_with_password(&host, PORT, &user, &pass);
        assert!(r.is_ok());
        assert!(ssh.sftp_stat( "/home/support").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file").is_err());
        assert!(ssh.sftp_create("/home/support/file").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file").is_ok());
        assert!(ssh.sftp_rename("/home/support/file","/home/support/file1").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file").is_err());
        assert!(ssh.sftp_stat( "/home/support/file1").is_ok());
        assert!(ssh.sftp_delete( "/home/support/file1").is_ok());
        assert!(ssh.sftp_stat( "/home/support/file1").is_err());
    }
}