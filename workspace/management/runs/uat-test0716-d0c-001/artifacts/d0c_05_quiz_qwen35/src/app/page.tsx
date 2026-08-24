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
  const [currentQuestion, setCurrentQuestion] = useState<number>(-1);
  const [score, setScore] = useState<number>(0);
  const [selectedAnswer, setSelectedAnswer] = useState<number | null>(null);
  const [quizComplete, setQuizComplete] = useState<boolean>(false);

  const handleStart = () => {
    setCurrentQuestion(0);
    setScore(0);
    setSelectedAnswer(null);
    setQuizComplete(false);
  };

  const handleAnswer = (answerIndex: number) => {
    if (selectedAnswer !== null) return;
    setSelectedAnswer(answerIndex);
    if (answerIndex === questions[currentQuestion].answer) {
      setScore(score + 1);
    }
  };

  const handleNext = () => {
    if (currentQuestion < questions.length - 1) {
      setCurrentQuestion(currentQuestion + 1);
      setSelectedAnswer(null);
    } else {
      setQuizComplete(true);
    }
  };

  const stateSnapshot = JSON.stringify({
    status: quizComplete ? "complete" : currentQuestion >= 0 ? "playing" : "idle",
    currentQuestion: currentQuestion,
    score: score,
    selectedAnswer: selectedAnswer,
  });

  return (
    <div
      data-anvil-state={stateSnapshot}
      className="min-h-screen bg-gradient-to-br from-indigo-50 via-purple-50 to-pink-50 flex items-center justify-center p-4"
    >
      <div className="bg-white rounded-2xl shadow-xl p-8 max-w-md w-full">
        <h1 className="text-3xl font-bold text-center text-gray-800 mb-6">
          クイズアプリ
        </h1>

        {currentQuestion === -1 && (
          <div className="text-center">
            <p className="text-gray-600 mb-6">3問のクイズに挑戦しましょう！</p>
            <button
              data-anvil-action="primary"
              onClick={handleStart}
              className="bg-indigo-600 text-white px-6 py-3 rounded-lg text-lg font-semibold hover:bg-indigo-700 transition-colors"
            >
              スタート
            </button>
          </div>
        )}

        {currentQuestion >= 0 && !quizComplete && (
          <div>
            <p className="text-sm text-gray-500 mb-2">
              質問 {currentQuestion + 1} / {questions.length}
            </p>
            <h2 className="text-xl font-semibold text-gray-800 mb-4">
              {questions[currentQuestion].question}
            </h2>
            <div className="space-y-3">
              {questions[currentQuestion].options.map((option, index) => (
                <button
                  key={index}
                  data-anvil-action="primary"
                  onClick={() => handleAnswer(index)}
                  disabled={selectedAnswer !== null}
                  className={`w-full text-left p-4 rounded-lg border-2 transition-colors ${
                    selectedAnswer === index
                      ? index === questions[currentQuestion].answer
                        ? "border-green-500 bg-green-50"
                        : "border-red-500 bg-red-50"
                      : selectedAnswer !== null &&
                        index === questions[currentQuestion].answer
                      ? "border-green-500 bg-green-50"
                      : "border-gray-200 hover:border-indigo-300"
                  }`}
                >
                  {option}
                </button>
              ))}
            </div>
            {selectedAnswer !== null && (
              <div className="mt-6 text-center">
                <button
                  onClick={handleNext}
                  className="bg-indigo-600 text-white px-6 py-3 rounded-lg text-lg font-semibold hover:bg-indigo-700 transition-colors"
                >
                  {currentQuestion < questions.length - 1
                    ? "次の問題へ"
                    : "結果を見る"}
                </button>
              </div>
            )}
          </div>
        )}

        {quizComplete && (
          <div className="text-center">
            <h2 className="text-2xl font-bold text-gray-800 mb-4">
              結果発表
            </h2>
            <p className="text-4xl font-bold text-indigo-600 mb-6">
              {score} / {questions.length}
            </p>
            <p className="text-gray-600 mb-6">
              {score === questions.length
                ? "素晴らしい！全問正解です！"
                : score >= questions.length / 2
                ? "よくできました！"
                : "もう一度挑戦してみましょう！"}
            </p>
            <button
              data-anvil-action="restart"
              onClick={handleStart}
              className="bg-indigo-600 text-white px-6 py-3 rounded-lg text-lg font-semibold hover:bg-indigo-700 transition-colors"
            >
              もう一度プレイ
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
