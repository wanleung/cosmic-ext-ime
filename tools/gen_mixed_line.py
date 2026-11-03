"""Build an 88-word English/Chinese sentence as vktap keystrokes.

Chinese segments are typed as standard (Da Chen) Zhuyin and committed with
Enter; English segments are typed after a Caps Lock that switches the engine
to its English sub-mode (and back before the next Chinese segment).  Prints
one JSON object: the evdev key list for tools/run_line.sh, the expected text,
and some counts.
"""

import json

# evdev keycodes on a US layout.
EV = {
    '1': 2, '2': 3, '3': 4, '4': 5, '5': 6, '6': 7, '7': 8, '8': 9, '9': 10, '0': 11,
    '-': 12, ';': 39, ',': 51, '.': 52, '/': 53,
    'q': 16, 'w': 17, 'e': 18, 'r': 19, 't': 20, 'y': 21, 'u': 22, 'i': 23, 'o': 24, 'p': 25,
    'a': 30, 's': 31, 'd': 32, 'f': 33, 'g': 34, 'h': 35, 'j': 36, 'k': 37, 'l': 38,
    'z': 44, 'x': 45, 'c': 46, 'v': 47, 'b': 48, 'n': 49, 'm': 50,
    ' ': 57, '!': 2,
}
CAPS, ENTER, SHIFT = 58, 28, 42

# Text -> Zhuyin keys; ' ' is the first tone.
ZH = {
    '今天': 'rup wu0 ', '天氣': 'wu0 fu4', '很好': 'cp3cl3', '我': 'ji3', '測試': 'hk4g4',
    '中文': '5j/ jp6', '輸入法': 'gj bj4z83', '電腦': '2u04sl3', '看': 'd04', '可以': 'dk3u3',
    '打字': '283y4', '英文': 'u/ jp6', '現在': 'vu04y94', '我們': 'ji3ap6', '一起': 'u fu3',
    '學習': 'vm,6vu6', '朋友': 'q/6u.3', '喜歡': 'vu3cj0 ', '音樂': 'up m,4', '吃飯': 't z04',
    '喝水': 'ck gjo3', '工作': 'ej/ yji4', '時間': 'g6ru0 ', '明天': 'au/6wu0 ', '早安': 'yl30 ',
    '晚安': 'j030 ', '沒問題': 'ao6jp4wu6', '謝謝你': 'vu,4vu,4su3', '再見': 'y94ru04',
    '你好': 'su3cl3', '太好了': 'w94cl3xk7',
}

LINE = [
    ('en', 'Today morning '), ('zh', '今天'), ('en', ' the weather outside '), ('zh', '天氣'),
    ('en', ' is really '), ('zh', '很好'), ('en', ' so now I '), ('zh', '我'),
    ('en', ' want to test '), ('zh', '測試'), ('en', ' this new '), ('zh', '中文'),
    ('en', ' input method quickly '), ('zh', '輸入法'), ('en', ' on my old '), ('zh', '電腦'),
    ('en', ' and then see '), ('zh', '看'), ('en', ' if it can also '), ('zh', '可以'),
    ('en', ' type and mix '), ('zh', '打字'), ('en', ' both '), ('zh', '英文'),
    ('en', ' and '), ('zh', '中文'), ('en', ' in one single line. '), ('zh', '現在'),
    ('en', ' we '), ('zh', '我們'), ('en', ' are '), ('zh', '一起'),
    ('en', ' learning '), ('zh', '學習'), ('en', ' with '), ('zh', '朋友'),
    ('en', ' who '), ('zh', '喜歡'), ('en', ' music '), ('zh', '音樂'),
    ('en', ', then '), ('zh', '吃飯'), ('en', ' and '), ('zh', '喝水'),
    ('en', ' before '), ('zh', '工作'), ('en', '. '), ('zh', '時間'),
    ('en', ' flies, '), ('zh', '明天'), ('en', ' again. '), ('zh', '早安'),
    ('en', ' everyone, '), ('zh', '晚安'), ('en', ' too. '), ('zh', '沒問題'),
    ('en', ', '), ('zh', '謝謝你'), ('en', ' and '), ('zh', '再見'),
    ('en', '! '), ('zh', '你好'), ('en', ' world '), ('zh', '太好了'),
]


def english_key(ch):
    if ch.isupper():
        return f'{SHIFT}+{EV[ch.lower()]}'
    if ch == '!':
        return f'{SHIFT}+{EV["1"]}'
    return str(EV[ch])


def main():
    keys = []
    expected = ''
    english = False
    for kind, text in LINE:
        if (kind == 'en') != english:
            keys.append(str(CAPS))
            english = not english
        if kind == 'en':
            keys.extend(english_key(ch) for ch in text)
        else:
            keys.extend(str(EV[ch]) for ch in ZH[text])
            keys.append(str(ENTER))
        expected += text
    words = sum(len(t.split()) if k == 'en' else 1 for k, t in LINE)
    print(json.dumps({
        'keys': ' '.join(keys),
        'expected': expected,
        'nkeys': len(keys),
        'words': words,
        'segments': len(LINE),
    }, ensure_ascii=False))


if __name__ == '__main__':
    main()
