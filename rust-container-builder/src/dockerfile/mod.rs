use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    From {
        image: String,
        alias: Option<String>,
    },
    Run {
        command: String,
    },
    Cmd {
        command: Vec<String>,
    },
    Label {
        key: String,
        value: String,
    },
    Env {
        key: String,
        value: String,
    },
    Copy {
        src: Vec<String>,
        dest: String,
        from: Option<String>, // For --from flag
    },
    Add {
        src: Vec<String>,
        dest: String,
    },
    Workdir {
        path: String,
    },
    Expose {
        port: u16,
    },
    Entrypoint {
        command: Vec<String>,
    },
    Volume {
        volumes: Vec<String>,
    },
    User {
        user: String,
    },
    Arg {
        key: String,
        default: Option<String>,
    },
    Onbuild {
        instruction: Box<Instruction>,
    },
    StopSignal {
        signal: String,
    },
    Healthcheck {
        interval: Option<u64>,
        timeout: Option<u64>,
        start_period: Option<u64>,
        retries: Option<u32>,
        cmd: Vec<String>,
    },
    Shell {
        shell: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ParsedDockerfile {
    pub stages: Vec<BuildStage>,
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct BuildStage {
    pub name: Option<String>,
    pub base_image: String,
    pub instructions: Vec<Instruction>,
}

pub struct DockerfileParser;

impl DockerfileParser {
    pub async fn parse_from_path<P: AsRef<Path>>(path: P) -> Result<ParsedDockerfile> {
        let content = tokio::fs::read_to_string(path).await?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<ParsedDockerfile> {
        let mut instructions = Vec::new();
        let mut args = HashMap::new();
        
        // Split content into lines and process
        let lines: Vec<&str> = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        for line in lines {
            let instruction = Self::parse_line(line)?;
            if let Instruction::Arg { key, default } = &instruction {
                // Handle ARG instructions by storing defaults
                if let Some(default_val) = default {
                    args.insert(key.clone(), default_val.clone());
                }
            }
            instructions.push(instruction);
        }

        // Group instructions into stages based on FROM commands
        let stages = Self::group_into_stages(instructions);

        Ok(ParsedDockerfile { stages, args })
    }

    fn parse_line(line: &str) -> Result<Instruction> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(Instruction::Run { command: "".to_string() });
        }

        let instruction = parts[0].to_uppercase();
        let args_str = line[line.find(' ').unwrap_or(line.len())..].trim_start();

        match instruction.as_str() {
            "FROM" => Self::parse_from(args_str),
            "RUN" => Ok(Instruction::Run {
                command: args_str.to_string(),
            }),
            "CMD" => Ok(Self::parse_cmd(args_str)?),
            "LABEL" => Ok(Self::parse_label(args_str)?),
            "ENV" => Ok(Self::parse_env(args_str)?),
            "COPY" => Ok(Self::parse_copy(args_str)),
            "ADD" => Ok(Self::parse_add(args_str)),
            "WORKDIR" => Ok(Instruction::Workdir {
                path: args_str.to_string(),
            }),
            "EXPOSE" => Ok(Self::parse_expose(args_str)?),
            "ENTRYPOINT" => Ok(Self::parse_entrypoint(args_str)?),
            "VOLUME" => Ok(Self::parse_volume(args_str)),
            "USER" => Ok(Instruction::User {
                user: args_str.to_string(),
            }),
            "ARG" => Ok(Self::parse_arg(args_str)?),
            "ONBUILD" => Ok(Self::parse_onbuild(args_str)?),
            "STOPSIGNAL" => Ok(Instruction::StopSignal {
                signal: args_str.to_string(),
            }),
            "HEALTHCHECK" => Ok(Self::parse_healthcheck(args_str)),
            "SHELL" => Ok(Self::parse_shell(args_str)),
            _ => Ok(Instruction::Run {
                command: line.to_string(),
            }), // Default to RUN for unknown instructions
        }
    }

    fn parse_from(args: &str) -> Result<Instruction> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("FROM instruction requires an image"));
        }

        let image = parts[0].to_string();
        let alias = if parts.len() > 2 && parts[1].to_uppercase() == "AS" {
            Some(parts[2].to_string())
        } else {
            None
        };

        Ok(Instruction::From { image, alias })
    }

    fn parse_cmd(args: &str) -> Result<Instruction> {
        // For simplicity, treat everything as a shell command
        // In a real implementation, we'd distinguish between exec and shell form
        Ok(Instruction::Cmd {
            command: vec!["/bin/sh".to_string(), "-c".to_string(), args.to_string()],
        })
    }

    fn parse_label(args: &str) -> Result<Instruction> {
        let parts: Vec<&str> = args.split('=').collect();
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("LABEL requires key=value format"));
        }

        Ok(Instruction::Label {
            key: parts[0].to_string(),
            value: parts[1..].join("="),
        })
    }

    fn parse_env(args: &str) -> Result<Instruction> {
        let parts: Vec<&str> = args.split('=').collect();
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("ENV requires key=value format"));
        }

        Ok(Instruction::Env {
            key: parts[0].to_string(),
            value: parts[1..].join("="),
        })
    }

    fn parse_copy(args: &str) -> Instruction {
        // Simplified parsing - in reality, COPY supports many flags
        let mut src_dest = args.split_whitespace().collect::<Vec<_>>();
        if src_dest.len() < 2 {
            return Instruction::Copy {
                src: vec![],
                dest: "".to_string(),
                from: None,
            };
        }

        let dest = src_dest.pop().unwrap().to_string();
        let src = src_dest.iter().map(|s| s.to_string()).collect();

        Instruction::Copy {
            src,
            dest,
            from: None, // Would need more complex parsing for --from flag
        }
    }

    fn parse_add(args: &str) -> Instruction {
        let parts: Vec<&str> = args.split_whitespace().collect();
        if parts.len() < 2 {
            return Instruction::Add {
                src: vec![],
                dest: "".to_string(),
            };
        }

        let dest = parts.last().unwrap().to_string();
        let src: Vec<String> = parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect();

        Instruction::Add { src, dest }
    }

    fn parse_expose(args: &str) -> Result<Instruction> {
        let port = args.trim().parse::<u16>()?;
        Ok(Instruction::Expose { port })
    }

    fn parse_entrypoint(args: &str) -> Result<Instruction> {
        // For simplicity, treat everything as a shell command
        Ok(Instruction::Entrypoint {
            command: vec!["/bin/sh".to_string(), "-c".to_string(), args.to_string()],
        })
    }

    fn parse_volume(args: &str) -> Instruction {
        let volumes: Vec<String> = args
            .split_whitespace()
            .map(|s| s.trim_matches('"').to_string())
            .collect();

        Instruction::Volume { volumes }
    }

    fn parse_arg(args: &str) -> Result<Instruction> {
        let parts: Vec<&str> = args.split('=').collect();
        let key = parts[0].to_string();
        let default = if parts.len() > 1 {
            Some(parts[1..].join("="))
        } else {
            None
        };

        Ok(Instruction::Arg { key, default })
    }

    fn parse_onbuild(args: &str) -> Result<Instruction> {
        let inner_instruction = Self::parse_line(args)?;
        Ok(Instruction::Onbuild {
            instruction: Box::new(inner_instruction),
        })
    }

    fn parse_healthcheck(args: &str) -> Instruction {
        // Simplified - real parsing would handle all flags
        Instruction::Healthcheck {
            interval: None,
            timeout: None,
            start_period: None,
            retries: None,
            cmd: vec!["CMD-SHELL".to_string(), args.to_string()],
        }
    }

    fn parse_shell(args: &str) -> Instruction {
        let parts: Vec<String> = args
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        Instruction::Shell { shell: parts }
    }

    fn group_into_stages(instructions: Vec<Instruction>) -> Vec<BuildStage> {
        let mut stages = Vec::new();
        let mut current_stage_instructions = Vec::new();
        let mut current_base_image = String::new();

        for instruction in instructions {
            if let Instruction::From { image, alias } = instruction {
                // Save previous stage if it exists
                if !current_stage_instructions.is_empty() {
                    stages.push(BuildStage {
                        name: alias,
                        base_image: current_base_image,
                        instructions: current_stage_instructions,
                    });
                }

                // Start new stage
                current_base_image = image;
                current_stage_instructions = Vec::new();
            } else {
                current_stage_instructions.push(instruction);
            }
        }

        // Add the final stage
        if !current_stage_instructions.is_empty() {
            stages.push(BuildStage {
                name: None, // Last stage has no alias unless explicitly named
                base_image: current_base_image,
                instructions: current_stage_instructions,
            });
        }

        stages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_dockerfile() {
        let dockerfile_content = r#"
        FROM alpine:latest
        RUN apk update
        RUN apk add curl
        CMD ["echo", "hello world"]
        "#;

        let parsed = DockerfileParser::parse(dockerfile_content).unwrap();
        assert_eq!(parsed.stages.len(), 1);
        assert_eq!(parsed.stages[0].instructions.len(), 3);
    }

    #[test]
    fn test_parse_multistage_dockerfile() {
        let dockerfile_content = r#"
        FROM golang:1.19 AS builder
        WORKDIR /app
        COPY . .
        RUN go build -o myapp .

        FROM alpine:latest
        RUN apk add --no-cache ca-certificates
        COPY --from=builder /app/myapp /myapp
        CMD ["/myapp"]
        "#;

        let parsed = DockerfileParser::parse(dockerfile_content).unwrap();
        assert_eq!(parsed.stages.len(), 2);
    }
}