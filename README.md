# UPBit API for YiPiR Service
가상자산, 주식 거래 및 알고리즘 기반 자동매매를 위한 프로그램

현재 AutoBit로 진행되는 프로젝트의 초기 버전입니다.\
간단하게 UPBit API 요청, 받아온 데이터에 대한 RSI, 가격 관련 연산이 구현되어 있습니다.\
아주 초기버전을 공개한 것이며 수정이나 업데이트는 없습니다.

## 간단 사용법
main.rs에서 업비트 모듈을 실행하는 함수가 예시로 작성되어 있고, mod.rs에 모든 내용이 들어있으니 적절히 수정해서 사용하시면 됩니다.\
아주 간단한 버전이지만 적절히 수정하면 본인이 취미로 자동매매 알고리즘 작성하기엔 충분할 거라 생각합니다.\


## mod.rs
서비스 메인 루프가 존재합니다. 여기서 판매 의사 및 구매 의사를 결정하는 코드와 조건 만족시 거래 API를 호출하는 코드가 있습니다.

## api.rs
UPBit API를 호출하는 함수들이 존재합니다. guaranteed_ 수식어가 붙은 함수는 UPBit API의 API 호출 횟수 제한으로 값을 받아오지 못할 경우를 대비하여, 여러 번 호출하더라도 확실히 값을 받아오는 것을 보장하는 함수들입니다.
수식어가 없는데도 같은 로직이 있는 함수가 있거나 해당 수식어 붙은 버전이 없는 함수도 있는 등 뒤죽박죽이니 적절히 참고하시길 바랍니다.
API 호출 및 반환값의 적절한 형 변환까지 하나의 builder pattern으로 진행됩니다.

## responses.rs
UPBit API에서 받은 데이터들의 반환형들을 정의하며, 해당 타입들에 대한 트레잇과 메소드들을 정의합니다.\
현재 여러 지표를 분석하는 메소드들도 CandleData 슬라이스에 대해 정의되어 사용하고 있습니다.

## ops.rs
CandleData를 인자로 받아 여러 지표를 연산하는 함수들이 정의되어 있습니다. 이 함수들의 여러 호출형이 responses.rs에 존재합니다.
