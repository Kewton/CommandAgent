"use client";

import { useState } from "react";

type Question = {
  question: string;
  options: string[];
  correctIndex: number;
};

const questions: Question[] = [
  {
    question: "日本の首都はどこですか？",
    options: ["大阪", "東京", "京都", "名古屋"],
    correctIndex: 1,
  },
  {
    question: "2 + 2 はいくつですか？",
    options: ["3", "5", "4", "6"],
    correctIndex: 2,
  },
  {
    question: "太陽系で最も大きい惑星は？",
    options: ["土星", "火星", "木星", "地球"],
    correctIndex: 2,
  },
];

export default function QuizPage() {
  const [screen, setScreen] = useState<"start" | "quiz" | "result">("start");
  const [currentQuestion, setCurrentQuestion] = useState(0);
  const [score, setScore] = useState(0);
  const [selectedAnswer, setSelectedAnswer] = useState<number | null>(null);

  const handleStart = () => {
    setScreen("quiz");
  };

  const handleAnswer = (index: number) => {
    setSelectedAnswer(index);
    if (index === questions[currentQuestion].correctIndex) {
      setScore((prev) => prev + 1);
    }
    setTimeout(() => {
      if (currentQuestion < questions.length - 1) {
        setCurrentQuestion((prev) => prev + 1);
        setSelectedAnswer(null);
      } else {
        setScreen("result");
      }
    }, 500);
  };

  const handleRestart = () => {
    setScreen("start");
    setCurrentQuestion(0);
    setScore(0);
    setSelectedAnswer(null);
  };

  const anvilState = JSON.stringify({
    screen,
    currentQuestion,
    score,
    showResult: screen === "result",
  });

  if (screen === "start") {
    return (
      <div
        className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100"
        data-anvil-state={anvilState}
      >
        <div className="bg-white p-8 rounded-xl shadow-lg text-center max-w-md w-full">
          <h1 className="text-3xl font-bold text-gray-800 mb-4">クイズアプリ</h1>
          <p className="text-gray-600 mb-8">3問の質問に答えて、スコアを競いましょう！</p>
          <button
            onClick={handleStart}
            className="px-8 py-3 bg-indigo-600 text-white text-lg font-semibold rounded-lg hover:bg-indigo-700 transition-colors shadow-md"
            data-anvil-action="primary"
          >
            スタート
          </button>
        </div>
      </div>
    );
  }

  if (screen === "quiz") {
    const question = questions[currentQuestion];
    return (
      <div
        className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100"
        data-anvil-state={anvilState}
      >
        <div className="bg-white p-8 rounded-xl shadow-lg max-w-md w-full">
          <div className="flex justify-between items-center mb-6">
            <span className="text-sm font-medium text-gray-500">
              質問 {currentQuestion + 1} / {questions.length}
            </span>
            <span className="text-sm font-medium text-indigo-600">
              スコア: {score}
            </span>
          </div>
          <h2 className="text-xl font-bold text-gray-800 mb-6">{question.question}</h2>
          <div className="space-y-4">
            {question.options.map((option, index) => (
              <button
                key={index}
                onClick={() => handleAnswer(index)}
                className={`w-full px-4 py-3 text-left rounded-lg transition-colors ${
                  selectedAnswer === index
                    ? index === question.correctIndex
                      ? "bg-green-500 text-white"
                      : "bg-red-500 text-white"
                    : "bg-gray-100 hover:bg-gray-200 text-gray-800"
                }`}
                data-anvil-action="input"
              >
                {option}
              </button>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (screen === "result") {
    return (
      <div
        className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100"
        data-anvil-state={anvilState}
      >
        <div className="bg-white p-8 rounded-xl shadow-lg text-center max-w-md w-full">
          <h1 className="text-3xl font-bold text-gray-800 mb-4">クイズ結果</h1>
          <p className="text-2xl font-semibold text-indigo-600 mb-6">
            {score} / {questions.length}
          </p>
          <p className="text-gray-600 mb-8">
            {score === questions.length
              ? "完璧です！素晴らしい！"
              : score > 0
              ? "よくできました！"
              : "もう一度挑戦してみましょう！"}
          </p>
          <button
            onClick={handleRestart}
            className="px-8 py-3 bg-indigo-600 text-white text-lg font-semibold rounded-lg hover:bg-indigo-700 transition-colors shadow-md"
            data-anvil-action="restart"
          >
            もう一度プレイ
          </button>
        </div>
      </div>
    );
  }

  return null;
}
