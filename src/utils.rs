use std::env;

/// conditionally print a message if an environment variable matches a string
/// this is intended for debugging purposes **NOTE**: Written by ChatGPT
/// ARGUMENTS:
/// env_var: &str: The name of the environment variable you want to check.
/// value_to_match: &str: The value you're comparing the environment variable against.
/// message_to_print: &str: The message that gets printed if the value of the environment variable matches the specified value. 
pub fn print_if_env_eq(env_var: &str, value_to_match: &str, message_to_print: &str) {
    // Retrieve the value of the environment variable
    if let Ok(value) = env::var(env_var) {
        // Check if the value matches the specified value_to_match
        if value == value_to_match {
            println!("{}", message_to_print);
        }
    }
}

