"use client";

import { useState } from "react";

type Question = {
  question: string;
  options: string[];
  answer: number;
};

const questions: Question[] = [
  {
    question: "日本の首都はどこですか？",
    options: ["大阪", "東京", "京都", "名古屋"],
    answer: 1,
  },
  {
    question: "2 + 2 は？",
    options: ["3", "4", "5", "6"],
    answer: 1,
  },
  {
    question: "太陽系で最大の惑星は？",
    options: ["地球", "火星", "木星", "土星"],
    answer: 2,
  },
];

export default function Page() {
  const [status, setStatus] = useState<"idle" | "playing" | "finished">("idle");
  const [currentQuestion, setCurrentQuestion] = useState(0);
  const [score, setScore] = useState(0);
  const [selectedAnswer, setSelectedAnswer] = useState<number | null>(null);
  const [showFeedback, setShowFeedback] = useState(false);

  const handleStart = () => {
    setStatus("playing");
    setCurrentQuestion(0);
    setScore(0);
    setSelectedAnswer(null);
    setShowFeedback(false);
  };

  const handleAnswer = (optionIndex: number) => {
    if (showFeedback) return;
    setSelectedAnswer(optionIndex);
    setShowFeedback(true);
    if (optionIndex === questions[currentQuestion].answer) {
      setScore((prev) => prev + 1);
    }
  };

  const handleNext = () => {
    if (currentQuestion + 1 < questions.length) {
      setCurrentQuestion((prev) => prev + 1);
      setSelectedAnswer(null);
      setShowFeedback(false);
    } else {
      setStatus("finished");
    }
  };

  const handleRestart = () => {
    setStatus("idle");
    setCurrentQuestion(0);
    setScore(0);
    setSelectedAnswer(null);
    setShowFeedback(false);
  };

  const stateSnapshot = JSON.stringify({
    status,
    questionIndex: status === "playing" ? currentQuestion : -1,
    score,
  });

  return (
    <div
      data-anvil-state={stateSnapshot}
      className="min-h-screen bg-gradient-to-br from-indigo-50 via-purple-50 to-pink-50 flex items-center justify-center p-4"
    >
      <div className="max-w-lg w-full bg-white/80 backdrop-blur-sm rounded-2xl shadow-xl p-8 space-y-6">
        {/* Header */}
        <div className="text-center">
          <h1 className="text-3xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-indigo-600 to-purple-600">
            クイズアプリ
          </h1>
          <p className="text-gray-500 mt-2">3問のクイズに挑戦しよう！</p>
        </div>

        {/* Idle State - Start Screen */}
        {status === "idle" && (
          <div className="space-y-6 text-center">
            <div className="text-6xl">🧠</div>
            <p className="text-gray-600">
              全3問のクイズ問題に挑戦します。
              <br />
              正解数を競いましょう！
            </p>
            <button
              data-anvil-action="primary"
              onClick={handleStart}
              className="w-full py-3 px-6 bg-gradient-to-r from-indigo-500 to-purple-600 text-white font-semibold rounded-xl shadow-lg hover:shadow-xl hover:scale-105 transition-all duration-200"
            >
              スタート
            </button>
          </div>
        )}

        {/* Playing State - Quiz */}
        {status === "playing" && (
          <div className="space-y-6">
            {/* Progress Bar */}
            <div className="flex items-center justify-between text-sm text-gray-500">
              <span>
                問題 {currentQuestion + 1} / {questions.length}
              </span>
              <span>スコア: {score}</span>
            </div>
            <div className="w-full bg-gray-200 rounded-full h-2">
              <div
                className="bg-gradient-to-r from-indigo-500 to-purple-600 h-2 rounded-full transition-all duration-300"
                style={{
                  width: `${((currentQuestion + 1) / questions.length) * 100}%`,
                }}
              />
            </div>

            {/* Question */}
            <div className="bg-gradient-to-r from-indigo-50 to-purple-50 rounded-xl p-6">
              <h2 className="text-xl font-semibold text-gray-800 text-center">
                {questions[currentQuestion].question}
              </h2>
            </div>

            {/* Options */}
            <div className="space-y-3">
              {questions[currentQuestion].options.map((option, index) => {
                const isSelected = selectedAnswer === index;
                const isCorrect = index === questions[currentQuestion].answer;
                let buttonClass =
                  "w-full py-3 px-4 text-left rounded-xl border-2 transition-all duration-200 font-medium ";

                if (showFeedback) {
                  if (isCorrect) {
                    buttonClass +=
                      "bg-green-100 border-green-500 text-green-700";
                  } else if (isSelected) {
                    buttonClass +=
                      "bg-red-100 border-red-500 text-red-700";
                  } else {
                    buttonClass +=
                      "bg-gray-50 border-gray-200 text-gray-400";
                  }
                } else {
                  buttonClass +=
                    "bg-white border-gray-200 text-gray-700 hover:border-indigo-400 hover:bg-indigo-50";
                }

                return (
                  <button
                    key={index}
                    data-anvil-action="primary"
                    onClick={() => handleAnswer(index)}
                    disabled={showFeedback}
                    className={buttonClass}
                  >
                    {option}
                  </button>
                );
              })}
            </div>

            {/* Next Button */}
            {showFeedback && (
              <button
                onClick={handleNext}
                className="w-full py-3 px-6 bg-gradient-to-r from-indigo-500 to-purple-600 text-white font-semibold rounded-xl shadow-lg hover:shadow-xl hover:scale-105 transition-all duration-200"
              >
                {currentQuestion + 1 < questions.length
                  ? "次の問題へ →"
                  : "結果を見る →"}
              </button>
            )}
          </div>
        )}

        {/* Finished State - Results */}
        {status === "finished" && (
          <div className="space-y-6 text-center">
            <div className="text-6xl">
              {score === 3 ? "🎉" : score >= 2 ? "👏" : "💪"}
            </div>
            <h2 className="text-2xl font-bold text-gray-800">
              結果発表！
            </h2>
            <p className="text-5xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-indigo-600 to-purple-600">
              {score} / {questions.length}
            </p>
            <p className="text-gray-500">
              {score === 3
                ? "完璧です！素晴らしい！"
                : score >= 2
                ? "よくできました！"
                : "もう一度挑戦してみましょう！"}
            </p>
            <button
              data-anvil-action="restart"
              onClick={handleRestart}
              className="w-full py-3 px-6 bg-gradient-to-r from-indigo-500 to-purple-600 text-white font-semibold rounded-xl shadow-lg hover:shadow-xl hover:scale-105 transition-all duration-200"
            >
              もう一度挑戦する
            </button>
          </div>
        )}

        {/* Footer */}
        <div className="text-center text-xs text-gray-400 pt-4">
          Made with ❤️ by Next.js
        </div>
      </div>
    </div>
  );
}
