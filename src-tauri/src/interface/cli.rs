use std::str::FromStr;

use clap::{Args, Parser, Subcommand};

use crate::app::runtime::{AgentaApp, BootstrapOptions, init_tracing};
use crate::error::{AppError, AppResult};
use crate::interface::response::{error, success, SuccessEnvelope};
use crate::service::{
    CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput, CreateVersionInput,
    SearchInput, TaskQuery, UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
};

#[derive(Debug, Parser)]
#[command(name = "agenta")]
#[command(about = "Agenta local task and context CLI")]
struct Cli {
    #[arg(long)]
    config: Option<std::path::PathBuf>,
    #[arg(long)]
    human: bool,
    #[command(subcommand)]
    command: TopLevelCommand,
}

#[derive(Debug, Subcommand)]
enum TopLevelCommand {
    #[command(subcommand)]
    Project(ProjectCommand),
    #[command(subcommand)]
    Version(VersionCommand),
    #[command(subcommand)]
    Task(TaskCommand),
    #[command(subcommand)]
    Note(NoteCommand),
    #[command(subcommand)]
    Attachment(AttachmentCommand),
    #[command(subcommand)]
    Search(SearchCommand),
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Create(ProjectCreateArgs),
    Get(ProjectRefArgs),
    List,
    Update(ProjectUpdateArgs),
}

#[derive(Debug, Subcommand)]
enum VersionCommand {
    Create(VersionCreateArgs),
    Get(VersionRefArgs),
    List(VersionListArgs),
    Update(VersionUpdateArgs),
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    Create(TaskCreateArgs),
    Get(TaskRefArgs),
    List(TaskListArgs),
    Update(TaskUpdateArgs),
}

#[derive(Debug, Subcommand)]
enum NoteCommand {
    Create(NoteCreateArgs),
    List(TaskRefArgs),
}

#[derive(Debug, Subcommand)]
enum AttachmentCommand {
    Create(AttachmentCreateArgs),
    Get(AttachmentRefArgs),
    List(TaskRefArgs),
}

#[derive(Debug, Subcommand)]
enum SearchCommand {
    Query(SearchQueryArgs),
}

#[derive(Debug, Args)]
struct ProjectCreateArgs {
    #[arg(long)]
    slug: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    description: Option<String>,
}

#[derive(Debug, Args)]
struct ProjectRefArgs {
    #[arg(long)]
    project: String,
}

#[derive(Debug, Args)]
struct ProjectUpdateArgs {
    #[arg(long)]
    project: String,
    #[arg(long)]
    slug: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long = "default-version")]
    default_version: Option<String>,
}

#[derive(Debug, Args)]
struct VersionCreateArgs {
    #[arg(long)]
    project: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    status: Option<String>,
}

#[derive(Debug, Args)]
struct VersionRefArgs {
    #[arg(long)]
    version: String,
}

#[derive(Debug, Args)]
struct VersionListArgs {
    #[arg(long)]
    project: Option<String>,
}

#[derive(Debug, Args)]
struct VersionUpdateArgs {
    #[arg(long)]
    version: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    status: Option<String>,
}

#[derive(Debug, Args)]
struct TaskCreateArgs {
    #[arg(long)]
    project: String,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    summary: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    priority: Option<String>,
    #[arg(long = "created-by")]
    created_by: Option<String>,
}

#[derive(Debug, Args)]
struct TaskRefArgs {
    #[arg(long)]
    task: String,
}

#[derive(Debug, Args)]
struct TaskListArgs {
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    status: Option<String>,
}

#[derive(Debug, Args)]
struct TaskUpdateArgs {
    #[arg(long)]
    task: String,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    summary: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    priority: Option<String>,
    #[arg(long = "updated-by")]
    updated_by: Option<String>,
}

#[derive(Debug, Args)]
struct NoteCreateArgs {
    #[arg(long)]
    task: String,
    #[arg(long)]
    content: String,
    #[arg(long = "created-by")]
    created_by: Option<String>,
}

#[derive(Debug, Args)]
struct AttachmentCreateArgs {
    #[arg(long)]
    task: String,
    #[arg(long)]
    path: std::path::PathBuf,
    #[arg(long)]
    kind: Option<String>,
    #[arg(long = "created-by")]
    created_by: Option<String>,
    #[arg(long)]
    summary: Option<String>,
}

#[derive(Debug, Args)]
struct AttachmentRefArgs {
    #[arg(long)]
    attachment: String,
}

#[derive(Debug, Args)]
struct SearchQueryArgs {
    #[arg(long = "text", alias = "query")]
    text: String,
    #[arg(long)]
    limit: Option<usize>,
}

pub async fn run() -> i32 {
    init_tracing();
    let cli = Cli::parse();

    let result = async {
        let app = AgentaApp::bootstrap(BootstrapOptions {
            config_path: cli.config.clone(),
        })
        .await?;
        execute(app, cli.command).await
    }
    .await;

    match result {
        Ok(envelope) => {
            print_success(&envelope, cli.human);
            0
        }
        Err(app_error) => {
            print_error(&app_error, cli.human);
            1
        }
    }
}

async fn execute(app: AgentaApp, command: TopLevelCommand) -> AppResult<SuccessEnvelope> {
    match command {
        TopLevelCommand::Project(command) => execute_project(app, command).await,
        TopLevelCommand::Version(command) => execute_version(app, command).await,
        TopLevelCommand::Task(command) => execute_task(app, command).await,
        TopLevelCommand::Note(command) => execute_note(app, command).await,
        TopLevelCommand::Attachment(command) => execute_attachment(app, command).await,
        TopLevelCommand::Search(command) => execute_search(app, command).await,
    }
}

async fn execute_project(app: AgentaApp, command: ProjectCommand) -> AppResult<SuccessEnvelope> {
    match command {
        ProjectCommand::Create(args) => {
            let project = app
                .service
                .create_project(CreateProjectInput {
                    slug: args.slug,
                    name: args.name,
                    description: args.description,
                })
                .await?;
            success(
                "project.create",
                project,
                "Created project".to_string(),
            )
        }
        ProjectCommand::Get(args) => {
            let project = app.service.get_project(&args.project).await?;
            success("project.get", project, "Loaded project")
        }
        ProjectCommand::List => {
            let projects = app.service.list_projects().await?;
            success(
                "project.list",
                &projects,
                format!("Listed {} project(s)", projects.len()),
            )
        }
        ProjectCommand::Update(args) => {
            let project = app
                .service
                .update_project(
                    &args.project,
                    UpdateProjectInput {
                        slug: args.slug,
                        name: args.name,
                        description: args.description,
                        status: parse_optional_enum(args.status)?,
                        default_version: args.default_version,
                    },
                )
                .await?;
            success("project.update", project, "Updated project")
        }
    }
}

async fn execute_version(app: AgentaApp, command: VersionCommand) -> AppResult<SuccessEnvelope> {
    match command {
        VersionCommand::Create(args) => {
            let version = app
                .service
                .create_version(CreateVersionInput {
                    project: args.project,
                    name: args.name,
                    description: args.description,
                    status: parse_optional_enum(args.status)?,
                })
                .await?;
            success("version.create", version, "Created version")
        }
        VersionCommand::Get(args) => {
            let version = app.service.get_version(&args.version).await?;
            success("version.get", version, "Loaded version")
        }
        VersionCommand::List(args) => {
            let versions = app.service.list_versions(args.project.as_deref()).await?;
            success(
                "version.list",
                &versions,
                format!("Listed {} version(s)", versions.len()),
            )
        }
        VersionCommand::Update(args) => {
            let version = app
                .service
                .update_version(
                    &args.version,
                    UpdateVersionInput {
                        name: args.name,
                        description: args.description,
                        status: parse_optional_enum(args.status)?,
                    },
                )
                .await?;
            success("version.update", version, "Updated version")
        }
    }
}

async fn execute_task(app: AgentaApp, command: TaskCommand) -> AppResult<SuccessEnvelope> {
    match command {
        TaskCommand::Create(args) => {
            let task = app
                .service
                .create_task(CreateTaskInput {
                    project: args.project,
                    version: args.version,
                    title: args.title,
                    summary: args.summary,
                    description: args.description,
                    status: parse_optional_enum(args.status)?,
                    priority: parse_optional_enum(args.priority)?,
                    created_by: args.created_by,
                })
                .await?;
            success("task.create", task, "Created task")
        }
        TaskCommand::Get(args) => {
            let task = app.service.get_task(&args.task).await?;
            success("task.get", task, "Loaded task")
        }
        TaskCommand::List(args) => {
            let tasks = app
                .service
                .list_tasks(TaskQuery {
                    project: args.project,
                    version: args.version,
                    status: parse_optional_enum(args.status)?,
                })
                .await?;
            success("task.list", &tasks, format!("Listed {} task(s)", tasks.len()))
        }
        TaskCommand::Update(args) => {
            let task = app
                .service
                .update_task(
                    &args.task,
                    UpdateTaskInput {
                        version: args.version,
                        title: args.title,
                        summary: args.summary,
                        description: args.description,
                        status: parse_optional_enum(args.status)?,
                        priority: parse_optional_enum(args.priority)?,
                        updated_by: args.updated_by,
                    },
                )
                .await?;
            success("task.update", task, "Updated task")
        }
    }
}

async fn execute_note(app: AgentaApp, command: NoteCommand) -> AppResult<SuccessEnvelope> {
    match command {
        NoteCommand::Create(args) => {
            let activity = app
                .service
                .create_note(CreateNoteInput {
                    task: args.task,
                    content: args.content,
                    created_by: args.created_by,
                })
                .await?;
            success("note.create", activity, "Created note")
        }
        NoteCommand::List(args) => {
            let activities = app.service.list_notes(&args.task).await?;
            success(
                "note.list",
                &activities,
                format!("Listed {} note(s)", activities.len()),
            )
        }
    }
}

async fn execute_attachment(
    app: AgentaApp,
    command: AttachmentCommand,
) -> AppResult<SuccessEnvelope> {
    match command {
        AttachmentCommand::Create(args) => {
            let attachment = app
                .service
                .create_attachment(CreateAttachmentInput {
                    task: args.task,
                    path: args.path,
                    kind: parse_optional_enum(args.kind)?,
                    created_by: args.created_by,
                    summary: args.summary,
                })
                .await?;
            success("attachment.create", attachment, "Created attachment")
        }
        AttachmentCommand::Get(args) => {
            let attachment = app.service.get_attachment(&args.attachment).await?;
            success("attachment.get", attachment, "Loaded attachment")
        }
        AttachmentCommand::List(args) => {
            let attachments = app.service.list_attachments(&args.task).await?;
            success(
                "attachment.list",
                &attachments,
                format!("Listed {} attachment(s)", attachments.len()),
            )
        }
    }
}

async fn execute_search(app: AgentaApp, command: SearchCommand) -> AppResult<SuccessEnvelope> {
    match command {
        SearchCommand::Query(args) => {
            let result = app
                .service
                .search(SearchInput {
                    text: args.text,
                    limit: args.limit,
                })
                .await?;
            success("search.query", result, "Completed search")
        }
    }
}

fn parse_optional_enum<T>(value: Option<String>) -> AppResult<Option<T>>
where
    T: FromStr<Err = String>,
{
    value
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| AppError::InvalidArguments(error.to_string()))
        })
        .transpose()
}

fn print_success(envelope: &SuccessEnvelope, human: bool) {
    if human {
        println!("{}", envelope.summary);
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope.result).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".to_string())
        );
    }
}

fn print_error(app_error: &AppError, human: bool) {
    if human {
        eprintln!("{}", app_error.message());
    } else {
        let envelope = error(app_error);
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&envelope).unwrap_or_else(|_| "{}".to_string())
        );
    }
}
