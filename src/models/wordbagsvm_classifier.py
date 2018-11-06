#!/usr/bin/env python3

import argparse
import time
import math
import pickle
import sys
from typing import Dict, Any, List, Tuple, Iterable, cast, Union

from sklearn import svm

from models.tactic_predictor import TacticPredictor

from tokenizer import tokenizers
from data import get_text_data, encode_bag_classify_data, encode_bag_classify_input
from util import *
from serapi_instance import get_stem

class WordBagSVMClassifier(TacticPredictor):
    def load_saved_state(self, filename : str) -> None:
        with open(filename, 'rb') as checkpoint_file:
            checkpoint = pickle.load(checkpoint_file)
        assert checkpoint['stem-embeddings']
        assert checkpoint['tokenizer']
        assert checkpoint['classifier']
        assert checkpoint['options']

        self.embedding = checkpoint['stem-embeddings']
        self.tokenizer = checkpoint['tokenizer']
        self.classifier = checkpoint['classifier']
        self.options = checkpoint['options']
        pass

    def getOptions(self) -> List[Tuple[str, str]]:
        return self.options

    def __init__(self, options : Dict[str, Any]) -> None:
        assert options["filename"]
        self.load_saved_state(options["filename"])

    def predictDistribution(self, in_data : Dict[str, Union[str, List[str]]]) \
        -> torch.FloatTensor:
        goal = cast(str, in_data["goal"])
        feature_vector = encode_bag_classify_input(goal, self.tokenizer)
        distribution = self.classifier.predict_log_proba([feature_vector])
        return distribution

    def predictKTactics(self, in_data : Dict[str, Union[str, List[str]]], k : int) \
        -> List[Tuple[str, float]]:
        distribution = self.predictDistribution(in_data)
        probs_and_indices = list_topk(list(distribution), k)
        return [(self.embedding.decode_token(idx.data[0]) + ".",
                 math.exp(certainty.data[0]))
                for certainty, idx in probs_and_indices]

    def predictKTacticsWithLoss(self, in_data : Dict[str, Union[str, List[str]]], k : int,
                                correct : str) -> Tuple[List[Tuple[str, float]], float]:
        distribution = self.predictDistribution(in_data)
        stem = get_stem(correct)
        loss = 0
        probs_and_indices = list_topk(list(distribution), k)
        predictions = [(self.embedding.decode_token(idx.data[0]) + ".",
                        math.exp(certainty.data[0]))
                       for certainty, idx in probs_and_indices]
        return predictions, loss

Checkpoint = Tuple[Dict[Any, Any], float]

def main(args_list : List[str]) -> None:
    parser = argparse.ArgumentParser(description=
                                     "A second-tier predictor which predicts tactic "
                                     "stems based on word frequency in the goal")
    parser.add_argument("--context-filter", dest="context_filter",
                        type=str, default="default")
    parser.add_argument("--max-tuples", dest="max_tuples",
                        type=int, default=None)
    parser.add_argument("scrape_file")
    parser.add_argument("save_file")
    args = parser.parse_args(args_list)
    dataset = get_text_data(args.scrape_file, args.context_filter,
                            max_tuples=args.max_tuples, verbose=True)
    samples, tokenizer, embedding = encode_bag_classify_data(dataset,
                                                             tokenizers["no-fallback"],
                                                             100, 2)

    classifier = train(samples, embedding.num_tokens())

    state = {'stem-embeddings': embedding,
             'tokenizer':tokenizer,
             'classifier': classifier,
             'options': [
                 ("dataset size", str(len(samples))),
                 ("context filter", args.context_filter),
             ]}
    with open(args.save_file, 'wb') as f:
        pickle.dump(state, f)

def train(dataset, num_stems: int) -> Checkpoint:
    curtime = time.time()
    print("Training SVM...", end="")
    sys.stdout.flush()

    inputs, outputs = zip(*dataset)
    model = svm.SVC(gamma='scale', probability=True)
    model.fit(inputs, outputs)
    print(" {:.2f}s".format(time.time() - curtime))
    return model
