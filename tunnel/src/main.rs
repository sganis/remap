mod ssh;
mod command;


fn main() -> Result<(), String>{
    let mut ssh = ssh::Ssh::new();
    let pkey = String::from(ssh::Ssh::private_key_path().to_string_lossy());
    let user = "support";
    let host = "localhost";
    // let host = "192.168.100.202";
    let port = 22;

    match ssh.connect_with_key(host, port, user, &pkey) {
        Err(e) => {
            println!("{e}");
            Err(e)
        },        
        Ok(_) => {
            println!("Connected");
            let output = ssh.run("whoami").unwrap();
            println!("{}", output);

            match ssh.direct_tcpip("localhost", 7001, "localhost", 7001) {
                Ok(_) => (),
                Err(e) => println!("Error: {e}"),
            }


            Ok(())
        }
    }   
}


