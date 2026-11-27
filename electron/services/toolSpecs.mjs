// The tool catalogue: what the model may call, and the one-line title the
// transcript shows. Ported from services/agent/tools/specs.rs.

const object = (properties, required = []) => ({ type: "object", properties, required });

/** Every built-in tool, keyed by name. */
export const TOOLS = {
  web_search: {
    title: "Search the web",
    group: "web",
    risky: false,
    description:
      "Search the web. Use it for recent events, prices, versions, or facts you are not sure about. Results come back numbered; cite them as [n].",
    parameters: object(
      {
        query: { type: "string", description: "What to search for, in a few words." },
        max_results: { type: "integer", description: "How many results, 1 to 8. Default 5." },
      },
      ["query"],
    ),
  },
  read_url: {
    title: "Read a page",
    group: "web",
    risky: false,
    description: "Fetch one web page and read its text. Use it after a search, or when the user gives a link.",
    parameters: object({ url: { type: "string", description: "The full http or https address." } }, ["url"]),
  },
  recall: {
    title: "Search notes and documents",
    group: "memory",
    risky: false,
    description:
      "Search the user's own notes and indexed documents. Use it when the answer may be in something they saved before.",
    parameters: object({ query: { type: "string", description: "Words to look for." } }, ["query"]),
  },
  save_note: {
    title: "Save a note",
    group: "memory",
    risky: true,
    description: "Save something worth keeping to the user's notes.",
    parameters: object(
      {
        title: { type: "string", description: "A short title." },
        content: { type: "string", description: "The note, in markdown." },
      },
      ["title", "content"],
    ),
  },
  read_file: {
    title: "Read a file",
    group: "files",
    risky: false,
    description: "Read a text file from the workspace. Paths are relative to the workspace root.",
    parameters: object({ path: { type: "string", description: "The file, relative to the workspace." } }, ["path"]),
  },
  write_file: {
    title: "Write a file",
    group: "files",
    risky: true,
    description: "Create or replace a text file in the workspace.",
    parameters: object(
      {
        path: { type: "string", description: "The file, relative to the workspace." },
        content: { type: "string", description: "The whole file content." },
      },
      ["path", "content"],
    ),
  },
  list_files: {
    title: "List files",
    group: "files",
    risky: false,
    description: "List the files in a workspace folder.",
    parameters: object({ path: { type: "string", description: "A folder inside the workspace. Empty for the top." } }),
  },
  run_command: {
    title: "Run a command",
    group: "shell",
    risky: true,
    description:
      "Run one terminal command in the workspace and read its output. Use it to check the result of work you did.",
    parameters: object(
      {
        command: { type: "string", description: "The command line to run." },
        cwd: { type: "string", description: "A folder inside the workspace to run in. Empty for the top." },
        timeout_seconds: { type: "integer", description: "Time limit, 1 to 300. Default 60." },
      },
      ["command"],
    ),
  },
  ask_user: {
    title: "Ask the user",
    group: "chat",
    risky: false,
    description:
      "Ask the user to choose, when you need a decision only they can make. Ask everything at once, not one question at a time.",
    parameters: object(
      {
        questions: {
          type: "array",
          description: "1 to 4 questions, asked together.",
          items: object(
            {
              id: { type: "string" },
              header: { type: "string", description: "One or two words, for example Clarify or Format." },
              question: { type: "string", description: "The question, in one sentence." },
              options: {
                type: "array",
                description: "2 to 6 answers.",
                items: object(
                  {
                    label: { type: "string", description: "1 to 5 words." },
                    description: { type: "string" },
                    recommended: { type: "boolean" },
                  },
                  ["label"],
                ),
              },
              multiple: { type: "boolean", description: "True when the user may pick more than one." },
            },
            ["question", "options"],
          ),
        },
      },
      ["questions"],
    ),
  },
  todo_write: {
    title: "Update the plan",
    group: "chat",
    risky: false,
    description: "Keep a short plan for a multi-step job, so the user can see progress.",
    parameters: object(
      {
        items: {
          type: "array",
          items: object(
            {
              content: { type: "string" },
              status: { type: "string", enum: ["pending", "in_progress", "completed"] },
            },
            ["content", "status"],
          ),
        },
      },
      ["items"],
    ),
  },
  subagent: {
    title: "Run a subagent",
    group: "chat",
    risky: false,
    description:
      "Hand one self-contained job to a helper that works on its own and reports back. Use it for a big, separable piece of reading or checking.",
    parameters: object(
      {
        title: { type: "string", description: "A few words naming the job." },
        task: { type: "string", description: "Everything the subagent needs to know." },
      },
      ["title", "task"],
    ),
  },
  learn_skill: {
    title: "Save a skill",
    group: "skills",
    risky: true,
    description: "Save a reusable set of instructions as a skill the user can run again.",
    parameters: object(
      {
        name: { type: "string", description: "A short name, for example Weekly report." },
        description: { type: "string", description: "One line: when to use it." },
        instructions: { type: "string", description: "The full instructions in markdown." },
      },
      ["name", "description", "instructions"],
    ),
  },
};

/** Which tools a chat offers, from its switches and approval mode. */
export function toolsFor({ useWeb, useRecall, useFiles, useSkills, useShell, useSubagents = false, allowWeb = true }) {
  const names = [];
  if (useWeb && allowWeb) names.push("web_search", "read_url");
  if (useRecall) names.push("recall");
  if (useFiles) names.push("read_file", "write_file", "list_files", "save_note");
  if (useSkills) names.push("learn_skill");
  if (useShell) names.push("run_command");
  if (useSubagents) names.push("subagent");
  names.push("ask_user", "todo_write");
  return names.filter((n) => n in TOOLS);
}

/** The OpenAI tool schemas for a set of names. */
export const schemasFor = (names) =>
  names.map((name) => ({
    type: "function",
    function: { name, description: TOOLS[name].description, parameters: TOOLS[name].parameters },
  }));

