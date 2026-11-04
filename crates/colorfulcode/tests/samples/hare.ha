use fmt;
use os;
use strings;

// Maximum number of words to count.
def MAX_WORDS: size = 1024;

type word = struct {
	text: str,
	count: uint,
};

type error = !(void | strings::error);

// Finds a word in the list, returning its index.
fn find(words: []word, text: str) (size | void) = {
	for (let i = 0z; i < len(words); i += 1) {
		if (words[i].text == text) {
			return i;
		};
	};
};

export fn main() void = {
	let words: []word = [];
	defer free(words);
	const input = `hello world\nhello`;
	let iter = strings::tokenize(input, " ");
	for (true) {
		const tok = match (strings::next_token(&iter)) {
		case let s: str =>
			yield s;
		case done =>
			break;
		};
		match (find(words, tok)) {
		case let i: size =>
			words[i].count += 1;
		case void =>
			append(words, word { text = tok, count = 1 });
		};
	};
	fmt::printfln("{} unique words, first is '{}', hex {}", len(words), 'h', 0xFF)!;
};

@test fn empty() void = {
	assert(len([]: []word) == 0, "expected no words\t");
};
