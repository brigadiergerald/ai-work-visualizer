import {Component,type ReactNode} from 'react';
export default class SkinBoundary extends Component<{children:ReactNode;fallback:ReactNode;onFailure:(reason:string)=>void},{failed:boolean}>{
 state={failed:false};static getDerivedStateFromError(){return {failed:true};}
 componentDidCatch(){this.props.onFailure('SKIN_RENDERER_EXCEPTION');}
 render(){return this.state.failed?this.props.fallback:this.props.children;}
}
