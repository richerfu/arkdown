/*
 * Copyright (c) 2025 Huawei Device Co., Ltd.
 * Licensed under the Apache License, Version 2.0 (the "License");
 */

class AA {
  a = 1;
  test() {
    let cb = () => {
      this.a;
      new.target;
      print('cb');
    }
    print('test');
    return cb;
  }

  testVariableScopeRefrenceThis() {
    let cb = () => {
      for(let a = 1; a < 10; a++) {
        this.a;
        print('cb');
      }
    }
    print('test');
    return cb;
  }

  testVariableScopeReferenceTarget() {
    let cb = () => {
      for(let a = 1; a < 10; a++) {
        new.target;
        print('cb');
      }
    }
    print('test');
    return cb;
  }
}
