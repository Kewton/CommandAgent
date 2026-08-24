fn write_nextjs_profile_workspace(
    root: &Path,
    globals_css: Option<&str>,
    postcss_config: Option<&str>,
    tsconfig_json: Option<&str>,
) {
    std::fs::create_dir_all(root.join("src/app")).unwrap();
    std::fs::write(root.join("package.json"), nextjs_complete_package_json()).unwrap();
    let tsconfig_json = tsconfig_json.unwrap_or(nextjs_tsconfig_json());
    std::fs::write(root.join("tsconfig.json"), tsconfig_json).unwrap();
    std::fs::write(root.join("tailwind.config.ts"), nextjs_tailwind_config_ts()).unwrap();
    if let Some(postcss_config) = postcss_config {
        std::fs::write(root.join("postcss.config.js"), postcss_config).unwrap();
    }
    std::fs::write(
        root.join("src/app/page.tsx"),
        "export default function Page(){return <main className=\"min-h-screen\">App</main>;}",
    )
    .unwrap();
    std::fs::write(root.join("src/app/layout.tsx"), nextjs_layout_source()).unwrap();
    std::fs::write(
        root.join("src/app/global.d.ts"),
        "declare module \"*.css\";\n",
    )
    .unwrap();
    if let Some(globals_css) = globals_css {
        std::fs::write(root.join("src/app/globals.css"), globals_css).unwrap();
    }
}

fn generated_nextjs_artifact_plan_json(goal: &str) -> String {
    let expected_paths = nextjs_scaffold_expected_paths();
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "create-nextjs-artifacts".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Create a coherent Next.js scaffold with {}",
                expected_paths.join(", ")
            ),
            expected_paths,
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

fn generated_nextjs_fixture_plan_json_with_kind(
    goal: &str,
    check_path: &str,
    kind: &str,
) -> String {
    let mut expected_paths = vec![check_path.to_string()];
    if check_path.contains("scaffold") {
        expected_paths = nextjs_scaffold_expected_paths();
        expected_paths.push(check_path.to_string());
    }
    let verify = if kind == "setup" {
        Vec::new()
    } else {
        vec![format!("python3 -m py_compile {check_path}")]
    };
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "create-and-check-artifacts".to_string(),
            kind: kind.to_string(),
            expected_result: "pass".to_string(),
            instruction: format!(
                "Create the declared artifacts including {check_path} and keep the Next.js files coherent"
            ),
            expected_paths,
            verify,
        }],
    })
    .unwrap()
}

fn interactive_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useState } from "react";
export default function Page(){
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => setBullets((items) => [...items, { x: 10, y: 90 }]);
  const restart = () => {
setGameOver(false);
setScore(0);
setBullets([]);
setEnemies([{ x: 10, y: 20 }]);
  };
  useEffect(() => {
const onKeyDown = (event: KeyboardEvent) => {
  if (event.key === "ArrowLeft") {
    fireBullet();
  }
};
const frame = requestAnimationFrame(() => {
  bullets.forEach((bullet) => {
    enemies.forEach((enemy) => {
      if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
        setGameOver(true);
        setScore((value) => value + 10);
      }
    });
  });
  setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
});
window.addEventListener("keydown", onKeyDown);
return () => {
  cancelAnimationFrame(frame);
  window.removeEventListener("keydown", onKeyDown);
};
  }, [bullets, enemies]);
  return <main><button onClick={fireBullet}>Start</button><button onClick={restart}>Restart</button><canvas /><p>score {score} enemy collision {gameOver ? "game over" : "playing"}</p></main>;
}
"#
}

fn cross_file_weak_restart_interactive_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useRef, useState } from "react";
import { GameEngine } from "./gameEngine";
export default function Page(){
  const engineRef = useRef(new GameEngine());
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [screen, setScreen] = useState("gameOver");
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => {
setBullets((items) => [...items, { x: 10, y: 90 }]);
setScore((value) => value + 10);
  };
  const startGame = () => {
engineRef.current?.reset();
setScreen("playing");
setGameOver(false);
  };
  useEffect(() => {
const onKeyDown = (event: KeyboardEvent) => {
  if (event.key === "ArrowLeft") {
    fireBullet();
  }
};
const frame = requestAnimationFrame(() => {
  bullets.forEach((bullet) => {
    enemies.forEach((enemy) => {
      if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
        setGameOver(true);
        setScore((value) => value + 25);
      }
    });
  });
  setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
});
window.addEventListener("keydown", onKeyDown);
return () => {
  cancelAnimationFrame(frame);
  window.removeEventListener("keydown", onKeyDown);
};
  }, [bullets, enemies]);
  return <main><button onClick={startGame}>Restart</button><button onClick={fireBullet}>Fire</button><canvas /><p>score {score} enemy collision {gameOver ? "game over" : screen}</p></main>;
}
"#
}

fn cross_file_weak_restart_game_engine_source() -> &'static str {
    r#"export class GameEngine {
  score = 10;
  actors = [{ x: 1, y: 2 }];
  reset() {
this.score = 0;
this.actors = [{ x: 1, y: 2 }];
  }
}
"#
}

fn interactive_game_page_without_restart_source() -> &'static str {
    r#""use client";
import { useEffect, useState } from "react";
export default function Page(){
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => setBullets((items) => [...items, { x: 10, y: 90 }]);
  useEffect(() => {
const onKeyDown = (event: KeyboardEvent) => {
  if (event.key === "ArrowLeft") {
    fireBullet();
  }
};
const frame = requestAnimationFrame(() => {
  bullets.forEach((bullet) => {
    enemies.forEach((enemy) => {
      if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
        setGameOver(true);
        setScore((value) => value + 10);
      }
    });
  });
  setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
});
window.addEventListener("keydown", onKeyDown);
return () => {
  cancelAnimationFrame(frame);
  window.removeEventListener("keydown", onKeyDown);
};
  }, [bullets, enemies]);
  return <main><button onClick={fireBullet}>Fire</button><canvas /><p>score {score} enemy collision {gameOver ? "game over" : "playing"}</p></main>;
}
"#
}

fn contract_interactive_game_page_source() -> String {
    interactive_game_page_source()
        .replace(
            "<main>",
            r#"<main data-anvil-state={JSON.stringify({ score, gameOver, bulletCount: bullets.length, enemyCount: enemies.length })}>"#,
        )
        .replace(
            "<button onClick={fireBullet}>Start</button>",
            r#"<button data-anvil-action="primary" onClick={fireBullet}>Start</button>"#,
        )
        .replace(
            "<button onClick={restart}>Restart</button>",
            r#"<button data-anvil-action="restart" onClick={restart}>Restart</button>"#,
        )
}

fn contract_interactive_game_page_without_restart_source() -> String {
    interactive_game_page_without_restart_source()
        .replace(
            "<main>",
            r#"<main data-anvil-state={JSON.stringify({ score, gameOver, bulletCount: bullets.length, enemyCount: enemies.length })}>"#,
        )
        .replace(
            "<button onClick={fireBullet}>Fire</button>",
            r#"<button data-anvil-action="primary" onClick={fireBullet}>Fire</button>"#,
        )
}

fn overlay_only_restart_game_page_source() -> &'static str {
    r#""use client";
import { useEffect, useState } from "react";
export default function Page(){
  const [score, setScore] = useState(0);
  const [gameOver, setGameOver] = useState(false);
  const [screen, setScreen] = useState("menu");
  const [bullets, setBullets] = useState<{ x: number; y: number }[]>([]);
  const [enemies, setEnemies] = useState([{ x: 10, y: 20 }]);
  const fireBullet = () => {
setScreen("playing");
setBullets((items) => [...items, { x: 10, y: 90 }]);
setScore((value) => value + 1);
  };
  const restart = () => {
setGameOver(false);
setScreen("menu");
setScore(0);
setBullets([]);
setEnemies([{ x: 10, y: 20 }]);
  };
  useEffect(() => {
const onKeyDown = (event: KeyboardEvent) => {
  if (event.key === "ArrowLeft" || event.key === " ") fireBullet();
};
const frame = requestAnimationFrame(() => {
  bullets.forEach((bullet) => {
    enemies.forEach((enemy) => {
      if (Math.abs(bullet.x - enemy.x) < 12 && Math.abs(bullet.y - enemy.y) < 12) {
        setGameOver(true);
        setScore((value) => value + 10);
      }
    });
  });
  setEnemies((items) => items.map((enemy) => ({ ...enemy, x: enemy.x + 1 })));
});
window.addEventListener("keydown", onKeyDown);
return () => {
  cancelAnimationFrame(frame);
  window.removeEventListener("keydown", onKeyDown);
};
  }, [bullets, enemies]);
  return (
<main data-anvil-state={JSON.stringify({ screen, score, gameOver, bullets, enemies })}>
  <button data-anvil-action="primary" onClick={fireBullet}>Start</button>
  <canvas />
  <p>score {score} enemy collision {gameOver ? "game over" : screen}</p>
  {gameOver ? <button data-anvil-action="restart" onClick={restart}>Restart</button> : null}
</main>
  );
}
"#
}

fn generated_data_mutation_plan_json(goal: &str) -> String {
    serde_json::to_string(&StepPlan {
        goal: goal.to_string(),
        steps: vec![PlanStep {
            id: "mutate-input".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Mutate input/source.csv".to_string(),
            expected_paths: vec!["input/source.csv".to_string()],
            verify: Vec::new(),
        }],
    })
    .unwrap()
}

