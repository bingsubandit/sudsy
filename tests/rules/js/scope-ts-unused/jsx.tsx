import React, { useState } from "react";
//              ^^^^^^^^ unused-variable
import type { FC } from "react";
export const A: FC = () => <div />;
type Mapped = { [K in "a"]: 1 };
//   ^^^^^^ unused-variable
namespace B.C {}
declare module "m" { interface Ambient {} var ambient: number; }
